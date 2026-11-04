// SPDX-License-Identifier: CC-BY-4.0

//! eGON sealing: padding the loader image and scattering BROM pages.

use crate::checksum::image_checksum;
use alloc::vec;
use alloc::vec::Vec;

use super::header::{EGON_CHECKSUM_SEED, EgonHeader};

/// BROM's logical page size (awboot/`mksunxi` `BROM_PAGE_SIZE`).
///
/// BROM assumes every SPI flash has 2 KiB pages, so images for media with a larger page size must
/// scatter each 2 KiB of data into the first 2 KiB of a full page slot. The constant is the
/// logical chunk size for any medium, not an SPI-flash-only value.
pub const BROM_PAGE_SIZE: usize = 2048;

/// Padding for block devices (SD, eMMC, FEL).
pub const PADDING_BLOCK: usize = 512;

/// Padding for SPI flash.
pub const PADDING_FLASH: usize = 8192;

/// How much to pad a sealed image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    /// 512-byte padding for block devices (SD, eMMC, FEL).
    Block,
    /// 8192-byte padding for SPI flash.
    Flash,
}

impl Padding {
    /// The padding in bytes.
    pub const fn bytes(self) -> usize {
        match self {
            Padding::Block => PADDING_BLOCK,
            Padding::Flash => PADDING_FLASH,
        }
    }
}

/// The BROM page size assumed when sealing, in bytes.
///
/// Only values that are a multiple of [`BROM_PAGE_SIZE`] and at least as large can be built, so
/// [`seal`] never has to reject one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSize(usize);

impl PageSize {
    /// The 2 KiB pages BROM assumes for every medium.
    pub const BROM: PageSize = PageSize(BROM_PAGE_SIZE);

    /// A page size that is a multiple of [`BROM_PAGE_SIZE`] and at least as large.
    pub const fn new(bytes: usize) -> Result<Self, SealError> {
        if bytes < BROM_PAGE_SIZE || !bytes.is_multiple_of(BROM_PAGE_SIZE) {
            Err(SealError::BadPageSize)
        } else {
            Ok(PageSize(bytes))
        }
    }

    /// The page size in bytes.
    pub const fn bytes(self) -> usize {
        self.0
    }
}

/// How much to pad the sealed image and which BROM page size to assume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealOptions {
    padding: Padding,
    page_size: PageSize,
}

impl SealOptions {
    /// Block-device image: 512-byte padding, 2 KiB pages.
    pub const BLOCK: SealOptions = SealOptions::new(Padding::Block, PageSize::BROM);

    /// SPI-flash image: 8192-byte padding, 2 KiB pages.
    pub const FLASH: SealOptions = SealOptions::new(Padding::Flash, PageSize::BROM);

    /// Options from a padding strategy and a validated page size.
    pub const fn new(padding: Padding, page_size: PageSize) -> Self {
        SealOptions { padding, page_size }
    }
}

/// Why an image could not be sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// The image is shorter than the eGON header it must carry.
    TooShort,
    /// Page size must be a multiple of [`BROM_PAGE_SIZE`] and at least as large.
    BadPageSize,
}

/// Seal `image` (a loader binary carrying its eGON header) for BROM.
///
/// Returns the padded — and, when the page size exceeds 2 KiB, page-scattered — image with the
/// header's length and checksum fields filled in. The length is derived from `image.len()`, not
/// from the header's placeholder field; see ADR-0009.
pub fn seal(image: &[u8], options: SealOptions) -> Result<Vec<u8>, SealError> {
    if image.len() < EgonHeader::SIZE {
        return Err(SealError::TooShort);
    }

    let length = align_up(image.len(), options.padding.bytes());
    let mut buf = vec![0u8; length];
    buf[..image.len()].copy_from_slice(image);

    crate::bytes::write_u32_le(&mut buf, EgonHeader::LENGTH_OFFSET, length as u32);
    crate::bytes::write_u32_le(&mut buf, EgonHeader::CHECKSUM_OFFSET, EGON_CHECKSUM_SEED);
    let checksum = image_checksum(&buf);
    crate::bytes::write_u32_le(&mut buf, EgonHeader::CHECKSUM_OFFSET, checksum);

    Ok(scatter_pages(buf, options.page_size.bytes()))
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
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
