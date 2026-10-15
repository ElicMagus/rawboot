//! Byte-in/byte-out tests for eGON sealing, driven by independently computed vectors.

use rawboot_image::{BROM_PAGE_SIZE, PADDING_BLOCK, PADDING_FLASH, SealError, SealOptions, seal};

/// The 48-byte eGON header as laid down by the runtime, with placeholder checksum/length.
fn egon_header() -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(&0xEA00_000Au32.to_le_bytes());
    h.extend_from_slice(b"eGON.BT0");
    h.extend_from_slice(&0x1234_5678u32.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&0x30u32.to_le_bytes());
    h.extend_from_slice(&0x3030_3033u32.to_le_bytes());
    h.extend_from_slice(&0x0002_0000u32.to_le_bytes());
    h.extend_from_slice(&0x0002_8000u32.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x34, 0x2e, 0x30, 0x00]);
    assert_eq!(h.len(), 48);
    h
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

const BLOCK: SealOptions = SealOptions {
    padding: PADDING_BLOCK,
    page_size: BROM_PAGE_SIZE,
};
const FLASH: SealOptions = SealOptions {
    padding: PADDING_FLASH,
    page_size: BROM_PAGE_SIZE,
};

#[test]
fn block_padding_aligns_length_and_checksums() {
    let sealed = seal(&egon_header(), BLOCK).unwrap();

    assert_eq!(sealed.len(), 512);
    assert_eq!(read_u32(&sealed, 16), 512);
    assert_eq!(read_u32(&sealed, 12), 0xF812_D66D);
    assert!(sealed[48..].iter().all(|&b| b == 0));
}

#[test]
fn flash_padding_aligns_length_and_checksums() {
    let sealed = seal(&egon_header(), FLASH).unwrap();

    assert_eq!(sealed.len(), 8192);
    assert_eq!(read_u32(&sealed, 16), 8192);
    assert_eq!(read_u32(&sealed, 12), 0xF812_F46D);
    assert!(sealed[48..].iter().all(|&b| b == 0));
}

#[test]
fn large_pages_scatter_each_2kib_page_into_a_full_slot() {
    let pattern: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
    let sealed = seal(
        &pattern,
        SealOptions {
            padding: PADDING_FLASH,
            page_size: 4096,
        },
    )
    .unwrap();

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
    assert_eq!(seal(&[0u8; 47], BLOCK), Err(SealError::TooShort),);
}

#[test]
fn rejects_padding_that_is_neither_block_nor_flash() {
    assert_eq!(
        seal(
            &egon_header(),
            SealOptions {
                padding: 1024,
                page_size: BROM_PAGE_SIZE,
            },
        ),
        Err(SealError::BadPadding),
    );
}

#[test]
fn rejects_page_size_that_is_not_a_multiple_of_2kib() {
    assert_eq!(
        seal(
            &egon_header(),
            SealOptions {
                padding: PADDING_BLOCK,
                page_size: 1000,
            },
        ),
        Err(SealError::BadPageSize),
    );
}
