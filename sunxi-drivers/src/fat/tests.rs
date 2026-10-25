// SPDX-License-Identifier: Unlicense

//! Host tests for the block adapter and FAT writes, over an in-memory disk.

use std::vec;
use std::vec::Vec;

use core_io::{ErrorKind, Read, Seek, SeekFrom, Write};
use fatfs::{FormatVolumeOptions, format_volume};

use super::block::{BlockDevice, BlockError, BlockIo, SdCardIo};
use super::partition::{BLOCK_SIZE, Partition};
use super::{WriteError, create_or_replace_file, mount_partition, read_file};
use crate::sdcard::{SdError, Sdcard, Sdcard_data, sdmmc_blk_write};
use crate::sdhci::Sdhci;

const SECTOR: usize = BLOCK_SIZE as usize;

struct MockDisk {
    blocks: Vec<[u8; SECTOR]>,
    fail_read: Option<u64>,
    fail_write: Option<u64>,
}

impl MockDisk {
    fn new(sectors: usize) -> Self {
        Self {
            blocks: vec![[0u8; SECTOR]; sectors],
            fail_read: None,
            fail_write: None,
        }
    }
}

impl BlockIo for &mut MockDisk {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        let blocks = (buf.len() / SECTOR) as u64;
        if self.fail_read == Some(lba) {
            return Err(BlockError::Read { lba, blocks });
        }
        for index in 0..blocks as usize {
            let Some(block) = self.blocks.get(lba as usize + index) else {
                return Err(BlockError::Read { lba, blocks });
            };
            let at = index * SECTOR;
            buf[at..at + SECTOR].copy_from_slice(block);
        }
        Ok(())
    }

    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {
        let blocks = (buf.len() / SECTOR) as u64;
        if self.fail_write == Some(lba) {
            return Err(BlockError::Write { lba, blocks });
        }
        for index in 0..blocks as usize {
            let Some(block) = self.blocks.get_mut(lba as usize + index) else {
                return Err(BlockError::Write { lba, blocks });
            };
            let at = index * SECTOR;
            block.copy_from_slice(&buf[at..at + SECTOR]);
        }
        Ok(())
    }
}

fn partition_at(base_lba: u64, sectors: u32) -> Partition {
    Partition {
        base_lba,
        len_bytes: sectors as u64 * BLOCK_SIZE,
    }
}

fn format(mock: &mut MockDisk, partition: Partition) -> Partition {
    let mut device = BlockDevice::new(&mut *mock, partition);
    format_volume(
        &mut device,
        FormatVolumeOptions::new()
            .total_sectors((partition.len_bytes / BLOCK_SIZE) as u32)
            .bytes_per_cluster(BLOCK_SIZE as u32)
            .fats(1),
    )
    .expect("format the mock volume");
    device.flush().expect("flush the formatted volume");
    partition
}

#[test]
fn write_is_cached_until_flush() {
    let mut mock = MockDisk::new(8);
    let partition = partition_at(0, 8);

    {
        let mut device = BlockDevice::new(&mut mock, partition);
        assert_eq!(device.write(&[0x5a; 32]).unwrap(), 32);
    }

    assert!(
        mock.blocks[0].iter().all(|byte| *byte == 0),
        "an unflushed dirty block must not reach the card"
    );

    {
        let mut device = BlockDevice::new(&mut mock, partition);
        assert_eq!(device.write(&[0x5a; 32]).unwrap(), 32);
        device.seek(SeekFrom::Start(0)).unwrap();
        let mut echo = [0u8; 32];
        device.read_exact(&mut echo).unwrap();
        assert_eq!(echo, [0x5a; 32], "reads see the dirty cache");
        device.flush().unwrap();
    }

    assert_eq!(&mock.blocks[0][..32], &[0x5a; 32]);
    assert!(mock.blocks[0][32..].iter().all(|byte| *byte == 0));
}

#[test]
fn partial_writes_across_blocks_keep_their_bytes() {
    let mut mock = MockDisk::new(8);
    let partition = partition_at(0, 8);
    let mut data = [0u8; 700];
    for (index, byte) in data.iter_mut().enumerate() {
        *byte = index as u8;
    }

    {
        let mut device = BlockDevice::new(&mut mock, partition);
        device.seek(SeekFrom::Start(SECTOR as u64 - 10)).unwrap();
        device.write_all(&data).unwrap();
        device.flush().unwrap();
    }

    assert_eq!(&mock.blocks[0][SECTOR - 10..], &data[..10]);
    assert_eq!(&mock.blocks[1][..SECTOR], &data[10..10 + SECTOR]);
    assert_eq!(
        &mock.blocks[2][..data.len() - 10 - SECTOR],
        &data[10 + SECTOR..]
    );
    assert!(
        mock.blocks[2][data.len() - 10 - SECTOR..]
            .iter()
            .all(|byte| *byte == 0)
    );
}

#[test]
fn aligned_full_blocks_are_written_directly() {
    let mut mock = MockDisk::new(8);
    let partition = partition_at(0, 8);
    let data = [0xa5u8; SECTOR * 2];

    {
        let mut device = BlockDevice::new(&mut mock, partition);
        device.write_all(&data).unwrap();
    }

    assert_eq!(&mock.blocks[0][..], &data[..SECTOR]);
    assert_eq!(&mock.blocks[1][..], &data[SECTOR..]);
}

#[test]
fn flush_reports_a_card_write_failure() {
    let mut mock = MockDisk::new(8);
    mock.fail_write = Some(0);
    let partition = partition_at(0, 8);

    let mut device = BlockDevice::new(&mut mock, partition);
    device.write_all(&[1u8; 16]).unwrap();
    let err = device.flush().expect_err("the mock rejects the write");
    assert_eq!(err.kind(), ErrorKind::Other);
}

#[test]
fn create_read_replace_and_read_again() {
    let mut mock = MockDisk::new(256);
    let partition = format(&mut mock, partition_at(0, 256));
    let first = b"first payload bytes";
    let replacement = b"second, shorter";

    {
        let fs = mount_partition(&mut mock, partition).expect("mount formatted volume");
        create_or_replace_file(&fs, "payload.bin", first).expect("create payload.bin");
    }

    {
        let fs = mount_partition(&mut mock, partition).expect("remount after create");
        let root = fs.root_dir();
        let mut file = root.open_file("payload.bin").expect("payload.bin exists");
        let mut buf = [0u8; 64];
        let len = read_file(&mut file, &mut buf).expect("read payload.bin");
        assert_eq!(&buf[..len], first);
    }

    {
        let fs = mount_partition(&mut mock, partition).expect("remount before replace");
        create_or_replace_file(&fs, "payload.bin", replacement).expect("replace payload.bin");
    }

    {
        let fs = mount_partition(&mut mock, partition).expect("remount after replace");
        let root = fs.root_dir();
        let mut file = root
            .open_file("payload.bin")
            .expect("payload.bin still exists");
        let mut buf = [0u8; 64];
        let len = read_file(&mut file, &mut buf).expect("read replaced payload.bin");
        assert_eq!(&buf[..len], replacement);
    }
}

#[test]
fn create_fails_when_the_volume_is_full() {
    let mut mock = MockDisk::new(64);
    let partition = format(&mut mock, partition_at(0, 64));
    let data = vec![0x42u8; 256 * 1024];

    let fs = mount_partition(&mut mock, partition).expect("mount formatted volume");
    let err = create_or_replace_file(&fs, "too-big.bin", &data)
        .expect_err("a 256 KiB file cannot fit a 32 KiB volume");
    assert!(matches!(err, WriteError::Write(_)));
}

#[test]
fn partition_offset_is_applied_to_every_access() {
    let mut mock = MockDisk::new(8 + 256);
    let partition = partition_at(8, 256);
    format(&mut mock, partition);
    let data = b"volume starts after the MBR";

    let fs = mount_partition(&mut mock, partition).expect("mount volume at LBA 8");
    create_or_replace_file(&fs, "offset.bin", data).expect("write to the offset volume");
    drop(fs);

    assert!(
        mock.blocks[..8]
            .iter()
            .all(|block| block.iter().all(|byte| *byte == 0)),
        "nothing may be written before the partition start"
    );

    let fs = mount_partition(&mut mock, partition).expect("remount volume at LBA 8");
    let root = fs.root_dir();
    let mut file = root.open_file("offset.bin").expect("offset.bin exists");
    let mut buf = [0u8; 64];
    let len = read_file(&mut file, &mut buf).expect("read offset.bin");
    assert_eq!(&buf[..len], data);
}

#[test]
fn read_failure_is_reported() {
    let mut mock = MockDisk::new(8);
    mock.fail_read = Some(3);
    let partition = partition_at(0, 8);

    let mut device = BlockDevice::new(&mut mock, partition);
    device.seek(SeekFrom::Start(3 * BLOCK_SIZE)).unwrap();
    let mut buf = [0u8; 16];
    let err = device
        .read(&mut buf)
        .expect_err("the mock rejects the read");
    assert_eq!(err.kind(), ErrorKind::Other);
}

fn offline_card() -> Sdcard_data {
    Sdcard_data {
        card: Sdcard {
            version: 0,
            ocr: 0,
            rca: 0,
            cid: [0; 4],
            csd: [0; 4],
            extcsd: [0; 512],
            high_capacity: 0,
            tran_speed: 0,
            dsr_imp: 0,
            read_bl_len: SECTOR as u32,
            write_bl_len: SECTOR as u32,
            capacity: 0,
        },
        hci: Sdhci::default(),
        buf: [0; SECTOR],
        online: false,
    }
}

#[test]
fn offline_card_write_is_rejected() {
    let mut sd = offline_card();
    assert_eq!(
        sdmmc_blk_write(&mut sd, [0u8; 1].as_ptr(), 0, 1),
        Err(SdError::Offline)
    );

    let mut io = SdCardIo::new(&mut sd);
    assert_eq!(io.write_blocks(0, &[0u8; SECTOR]), Err(BlockError::Offline));
}
