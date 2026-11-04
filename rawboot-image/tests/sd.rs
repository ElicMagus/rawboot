// SPDX-License-Identifier: CC-BY-4.0

//! Byte-in/byte-out tests for the SD boot prefix: MBR and loader placement.

use rawboot_image::{
    DEFAULT_START_LBA, EgonHeader, MBR_SIGNATURE, PARTITION_ENTRY_SIZE, PARTITION_TABLE_OFFSET,
    PARTITION_TYPE_FAT32, SD_LOADER_OFFSET, SECTOR_SIZE, SdImageError, SdImageOptions, SealOptions,
    VerifyError, build_sd_image, read_u32_le, seal,
};

const DEFAULT_PARTITION_SECTORS: u32 = 1_843_200;

fn sealed_loader() -> Vec<u8> {
    seal(&EgonHeader::PLACEHOLDER.to_bytes(), SealOptions::BLOCK).unwrap()
}

fn options(start_lba: u32, partition_sectors: u32) -> SdImageOptions {
    SdImageOptions::new(start_lba, partition_sectors)
}

fn mbr_entry(image: &[u8]) -> &[u8] {
    &image[PARTITION_TABLE_OFFSET..PARTITION_TABLE_OFFSET + PARTITION_ENTRY_SIZE]
}

#[test]
fn builds_a_one_mib_prefix_with_the_mbr_and_loader() {
    let loader = sealed_loader();
    let image = build_sd_image(
        &loader,
        options(DEFAULT_START_LBA, DEFAULT_PARTITION_SECTORS),
    )
    .unwrap();

    assert_eq!(image.len(), DEFAULT_START_LBA as usize * SECTOR_SIZE);
    assert_eq!(&image[510..512], &MBR_SIGNATURE);
    assert!(image[440..446].iter().all(|&b| b == 0), "disk signature");

    let entry = mbr_entry(&image);
    assert_eq!(entry[0], 0x00, "boot flag");
    assert_eq!(&entry[1..4], &[0xFE, 0xFF, 0xFF], "start CHS");
    assert_eq!(entry[4], PARTITION_TYPE_FAT32);
    assert_eq!(&entry[5..8], &[0xFE, 0xFF, 0xFF], "end CHS");
    assert_eq!(read_u32_le(entry, 8), DEFAULT_START_LBA);
    assert_eq!(read_u32_le(entry, 12), DEFAULT_PARTITION_SECTORS);

    assert!(image[462..510].iter().all(|&b| b == 0), "extra entries");
    assert!(
        image[..440].iter().all(|&b| b == 0),
        "bootstrap code is zero"
    );

    let loader_at = SD_LOADER_OFFSET;
    assert_eq!(
        &image[loader_at..loader_at + loader.len()],
        loader.as_slice()
    );
    assert!(
        image[SECTOR_SIZE..loader_at].iter().all(|&b| b == 0),
        "sectors 1..16 are zero"
    );
    assert!(
        image[loader_at + loader.len()..].iter().all(|&b| b == 0),
        "tail is zero"
    );
}

#[test]
fn start_lba_sets_the_image_length_and_the_entry() {
    let loader = sealed_loader();
    let image = build_sd_image(&loader, options(4096, 100)).unwrap();

    assert_eq!(image.len(), 4096 * SECTOR_SIZE);
    let entry = mbr_entry(&image);
    assert_eq!(read_u32_le(entry, 8), 4096);
    assert_eq!(read_u32_le(entry, 12), 100);
}

#[test]
fn accepts_a_loader_that_ends_at_the_partition_start() {
    let loader = sealed_loader();
    let start_lba = (SD_LOADER_OFFSET + loader.len()) / SECTOR_SIZE;

    let image = build_sd_image(&loader, options(start_lba as u32, 1)).unwrap();

    assert_eq!(image.len(), start_lba * SECTOR_SIZE);
}

#[test]
fn rejects_a_loader_that_reaches_into_the_partition() {
    let loader = sealed_loader();

    assert_eq!(
        build_sd_image(&loader, options(16, 1)),
        Err(SdImageError::LoaderOverlapsPartition)
    );
}

#[test]
fn rejects_an_unsealed_loader() {
    let unsealed = EgonHeader::PLACEHOLDER.to_bytes();

    assert_eq!(
        build_sd_image(
            &unsealed,
            options(DEFAULT_START_LBA, DEFAULT_PARTITION_SECTORS)
        ),
        Err(SdImageError::LoaderNotSealed(VerifyError::BadLength))
    );
}

#[test]
fn rejects_an_empty_loader() {
    assert_eq!(
        build_sd_image(&[], options(DEFAULT_START_LBA, DEFAULT_PARTITION_SECTORS)),
        Err(SdImageError::LoaderEmpty)
    );
}

#[test]
fn rejects_zero_start_lba() {
    assert_eq!(
        build_sd_image(&sealed_loader(), options(0, DEFAULT_PARTITION_SECTORS)),
        Err(SdImageError::ZeroStartLba)
    );
}

#[test]
fn rejects_zero_partition_sectors() {
    assert_eq!(
        build_sd_image(&sealed_loader(), options(DEFAULT_START_LBA, 0)),
        Err(SdImageError::ZeroPartitionSectors)
    );
}

#[test]
fn rejects_an_overflowing_image_length() {
    assert_eq!(
        build_sd_image(&sealed_loader(), options(0x0080_0000, 1)),
        Err(SdImageError::SizeOverflow)
    );
}

#[test]
fn rejects_a_partition_that_ends_past_the_sector_address_width() {
    assert_eq!(
        build_sd_image(&sealed_loader(), options(2, u32::MAX)),
        Err(SdImageError::SizeOverflow)
    );
}
