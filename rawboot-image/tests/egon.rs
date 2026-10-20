// SPDX-License-Identifier: Unlicense

//! Byte-in/byte-out tests for eGON sealing.
//!
//! The fixture is the shared [`EgonHeader::PLACEHOLDER`] the runtime also lays down, so these
//! tests pin the sealing transform against independently computed checksum vectors.

use rawboot_image::{EgonHeader, Padding, PageSize, SealError, SealOptions, read_u32_le, seal};

/// The 48-byte eGON header as laid down by `rawboot-rt`, with placeholder checksum/length.
fn egon_header() -> Vec<u8> {
    EgonHeader::PLACEHOLDER.to_bytes().to_vec()
}

#[test]
fn block_padding_aligns_length_and_checksums() {
    let sealed = seal(&egon_header(), SealOptions::BLOCK).unwrap();

    assert_eq!(sealed.len(), 512);
    assert_eq!(read_u32_le(&sealed, EgonHeader::LENGTH_OFFSET), 512);
    assert_eq!(
        read_u32_le(&sealed, EgonHeader::CHECKSUM_OFFSET),
        0xF812_566D
    );
    assert!(sealed[48..].iter().all(|&b| b == 0));
}

#[test]
fn flash_padding_aligns_length_and_checksums() {
    let sealed = seal(&egon_header(), SealOptions::FLASH).unwrap();

    assert_eq!(sealed.len(), 8192);
    assert_eq!(read_u32_le(&sealed, EgonHeader::LENGTH_OFFSET), 8192);
    assert_eq!(
        read_u32_le(&sealed, EgonHeader::CHECKSUM_OFFSET),
        0xF812_746D
    );
    assert!(sealed[48..].iter().all(|&b| b == 0));
}

#[test]
fn large_pages_scatter_each_2kib_page_into_a_full_slot() {
    let pattern: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
    let options = SealOptions::new(Padding::Flash, PageSize::new(4096).unwrap());
    let sealed = seal(&pattern, options).unwrap();

    assert_eq!(sealed.len(), 16384);
    for page in 0..4usize {
        let slot = &sealed[page * 4096..(page + 1) * 4096];
        assert!(
            slot[2048..].iter().all(|&b| b == 0),
            "page {page} tail not zero"
        );
        for (offset, &byte) in slot.iter().take(2048).enumerate() {
            let source = page * 2048 + offset;
            let expected = if source < pattern.len() {
                source as u8
            } else {
                0
            };
            if page == 0 && (12..20).contains(&offset) {
                continue; // checksum/length fields are rewritten by seal
            }
            assert_eq!(byte, expected, "page {page} offset {offset}");
        }
    }
}

#[test]
fn rejects_image_smaller_than_header() {
    assert_eq!(
        seal(&[0u8; 47], SealOptions::BLOCK),
        Err(SealError::TooShort)
    );
}

#[test]
fn padding_reports_its_byte_count() {
    assert_eq!(Padding::Block.bytes(), 512);
    assert_eq!(Padding::Flash.bytes(), 8192);
}

#[test]
fn rejects_page_size_that_is_not_a_multiple_of_2kib() {
    assert_eq!(PageSize::new(1000), Err(SealError::BadPageSize));
}

#[test]
fn rejects_page_size_smaller_than_the_brom_page() {
    assert_eq!(PageSize::new(1024), Err(SealError::BadPageSize));
    assert_eq!(PageSize::new(0), Err(SealError::BadPageSize));
}
