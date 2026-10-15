// SPDX-License-Identifier: Unlicense

//! Allwinner eGON boot-image sealing.
//!
//! BROM validates an eGON image before running it: it checks that a 32-bit checksum over the
//! padded image matches the value in the header, with the checksum field seeded to
//! [`EGON_CHECKSUM_SEED`] before summing. [`seal`] reproduces the `mksunxi` transform used by
//! awboot: it pads the image to a block or flash boundary, overwrites the header's length field
//! with the padded length, computes the seeded checksum, and — for BROM page sizes larger than
//! 2 KiB — scatters each 2 KiB logical page into a zero-filled page slot.
//!
//! The input is the loader binary *with* its eGON header already laid down by the runtime; only
//! the length and checksum fields are modified. Unlike `mksunxi`, which sums
//! `ALIGN(header.length, padding)` words, `seal` derives the length from the actual input size —
//! the header field is an output here, so a stale placeholder in the binary cannot under- or
//! over-read the buffer.

use crate::checksum::sum_le_u32;
use alloc::vec;
use alloc::vec::Vec;

/// Length of the eGON header (`0x30`) as laid down by the runtime.
pub const EGON_HEADER_SIZE: usize = 48;

/// Offset of the checksum field within the eGON header.
pub const EGON_CHECKSUM_OFFSET: usize = 0x0C;

/// Offset of the length field within the eGON header.
pub const EGON_LENGTH_OFFSET: usize = 0x10;

/// Seed written into the checksum field before summing (awboot/`mksunxi`).
pub const EGON_CHECKSUM_SEED: u32 = 0x5F0A_6C39;

/// The page size BROM assumes for every SPI flash.
pub const BROM_PAGE_SIZE: usize = 2048;

/// Padding for block devices (SD, eMMC, FEL).
pub const PADDING_BLOCK: usize = 512;

/// Padding for SPI flash.
pub const PADDING_FLASH: usize = 8192;

/// How much to pad the sealed image and which BROM page size to assume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealOptions {
    pub padding: usize,
    pub page_size: usize,
}

impl SealOptions {
    /// Block-device image: 512-byte padding, 2 KiB pages.
    pub const BLOCK: SealOptions = SealOptions {
        padding: PADDING_BLOCK,
        page_size: BROM_PAGE_SIZE,
    };

    /// SPI-flash image: 8192-byte padding, 2 KiB pages.
    pub const FLASH: SealOptions = SealOptions {
        padding: PADDING_FLASH,
        page_size: BROM_PAGE_SIZE,
    };
}

/// Why an image could not be sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// The image is shorter than the eGON header it must carry.
    TooShort,
    /// Padding must be 512 (block) or 8192 (flash).
    BadPadding,
    /// Page size must be a multiple of 2048.
    BadPageSize,
}

/// Seal `image` (a loader binary carrying its eGON header) for BROM.
///
/// Returns the padded — and, when `options.page_size` exceeds 2 KiB, page-scattered — image with
/// the header's length and checksum fields filled in.
pub fn seal(image: &[u8], options: SealOptions) -> Result<Vec<u8>, SealError> {
    if image.len() < EGON_HEADER_SIZE {
        return Err(SealError::TooShort);
    }
    if options.padding != PADDING_BLOCK && options.padding != PADDING_FLASH {
        return Err(SealError::BadPadding);
    }
    if options.page_size < BROM_PAGE_SIZE || !options.page_size.is_multiple_of(BROM_PAGE_SIZE) {
        return Err(SealError::BadPageSize);
    }

    let length = align_up(image.len(), options.padding);
    let mut buf = vec![0u8; length];
    buf[..image.len()].copy_from_slice(image);

    write_u32(&mut buf, EGON_LENGTH_OFFSET, length as u32);
    write_u32(&mut buf, EGON_CHECKSUM_OFFSET, EGON_CHECKSUM_SEED);
    let checksum = sum_le_u32(&buf);
    write_u32(&mut buf, EGON_CHECKSUM_OFFSET, checksum);

    Ok(scatter_pages(buf, options.page_size))
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn scatter_pages(buf: Vec<u8>, page_size: usize) -> Vec<u8> {
    if page_size == BROM_PAGE_SIZE {
        return buf;
    }
    let pages = buf.len().div_ceil(BROM_PAGE_SIZE);
    let mut out = vec![0u8; pages * page_size];
    for (page, chunk) in buf.chunks(BROM_PAGE_SIZE).enumerate() {
        out[page * page_size..page * page_size + chunk.len()].copy_from_slice(chunk);
    }
    out
}
