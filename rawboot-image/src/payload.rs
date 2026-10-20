// SPDX-License-Identifier: Unlicense

//! Parsing of the rawboot payload image header.
//!
//! The payload ships as a raw binary behind a fixed 28-byte [`ImageHeader`]. `size` counts the
//! payload bytes that follow the header, and `checksum` is the wrapping sum of those bytes as
//! little-endian 32-bit words (a trailing partial word is zero-padded).
//!
//! [`parse_header`] lets the loader read `load_addr` and `size` before placing the payload;
//! [`parse`] then validates the checksum. [`build`] is the build tooling's seam: it wraps a raw
//! binary in the header, computing the checksum, so `xtask` emits images the loader accepts.

use crate::bytes::{read_u32_le, write_u32_le};
use crate::checksum::image_checksum;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const IMAGE_MAGIC: u32 = u32::from_le_bytes(*b"RBIF");
pub const IMAGE_VERSION: u32 = 1;
pub const IMAGE_HEADER_SIZE: usize = 28;

/// Where a payload defaults to being loaded when the header does not say otherwise.
pub const DEFAULT_LOAD_ADDR: u32 = 0x4200_0000;

/// Upper sanity bound on a payload's declared size (the T113-S3 DRAM size).
pub const MAX_IMAGE_SIZE: u32 = 128 * 1024 * 1024;

/// The fixed header at the front of a payload image.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHeader {
    pub magic: u32,
    pub version: u32,
    pub flags: u32,
    pub entry: u32,
    pub load_addr: u32,
    pub size: u32,
    pub checksum: u32,
}

mod offset {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 4;
    pub const FLAGS: usize = 8;
    pub const ENTRY: usize = 12;
    pub const LOAD_ADDR: usize = 16;
    pub const SIZE: usize = 20;
    pub const CHECKSUM: usize = 24;
}

impl ImageHeader {
    /// The header's little-endian byte representation, ready to prefix to a payload.
    pub const fn to_bytes(self) -> [u8; IMAGE_HEADER_SIZE] {
        let mut out = [0u8; IMAGE_HEADER_SIZE];
        write_u32_le(&mut out, offset::MAGIC, self.magic);
        write_u32_le(&mut out, offset::VERSION, self.version);
        write_u32_le(&mut out, offset::FLAGS, self.flags);
        write_u32_le(&mut out, offset::ENTRY, self.entry);
        write_u32_le(&mut out, offset::LOAD_ADDR, self.load_addr);
        write_u32_le(&mut out, offset::SIZE, self.size);
        write_u32_le(&mut out, offset::CHECKSUM, self.checksum);
        out
    }

    /// Read a header from the first [`IMAGE_HEADER_SIZE`] bytes of `bytes`, without validating it.
    const fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            magic: read_u32_le(bytes, offset::MAGIC),
            version: read_u32_le(bytes, offset::VERSION),
            flags: read_u32_le(bytes, offset::FLAGS),
            entry: read_u32_le(bytes, offset::ENTRY),
            load_addr: read_u32_le(bytes, offset::LOAD_ADDR),
            size: read_u32_le(bytes, offset::SIZE),
            checksum: read_u32_le(bytes, offset::CHECKSUM),
        }
    }
}

/// How [`build`] stamps a payload image header. `flags` is reserved for future features; leave it
/// zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageOptions {
    /// Where the payload should be placed in DRAM.
    pub load_addr: u32,
    /// Where execution should begin once the payload is placed.
    pub entry: u32,
    /// Reserved feature bits.
    pub flags: u32,
}

impl ImageOptions {
    /// Load and enter at `load_addr` from the start of the image, with no flags.
    pub const fn new(load_addr: u32) -> Self {
        Self {
            load_addr,
            entry: load_addr,
            flags: 0,
        }
    }
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self::new(DEFAULT_LOAD_ADDR)
    }
}

/// A parsed image: its header and the payload bytes it describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payload<'a> {
    pub header: ImageHeader,
    pub data: &'a [u8],
}

/// Why an image could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// The buffer is shorter than the header, or shorter than the declared payload.
    Truncated,
    /// The header magic does not match [`IMAGE_MAGIC`].
    BadMagic,
    /// The header version is not [`IMAGE_VERSION`].
    UnsupportedVersion,
    /// The declared size exceeds [`MAX_IMAGE_SIZE`].
    SizeTooLarge,
    /// The payload checksum does not match the header.
    BadChecksum,
}

impl core::fmt::Display for ImageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            ImageError::Truncated => "image truncated",
            ImageError::BadMagic => "bad image magic",
            ImageError::UnsupportedVersion => "unsupported image version",
            ImageError::SizeTooLarge => "declared image size too large",
            ImageError::BadChecksum => "image checksum mismatch",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for ImageError {}

/// Read and validate the header at the front of `image`, without verifying the payload checksum.
///
/// This is the loader's first step: it needs `load_addr` and `size` before it can place the
/// payload, and it verifies the checksum at the destination afterwards. The header's magic,
/// version and declared length are still checked here, so [`parse`] and [`parse_header`] reject
/// the same malformed headers.
pub fn parse_header(image: &[u8]) -> Result<ImageHeader, ImageError> {
    if image.len() < IMAGE_HEADER_SIZE {
        return Err(ImageError::Truncated);
    }

    let header = ImageHeader::from_bytes(image);

    if header.magic != IMAGE_MAGIC {
        return Err(ImageError::BadMagic);
    }
    if header.version != IMAGE_VERSION {
        return Err(ImageError::UnsupportedVersion);
    }
    if header.size > MAX_IMAGE_SIZE {
        return Err(ImageError::SizeTooLarge);
    }

    let end = IMAGE_HEADER_SIZE + header.size as usize;
    if end > image.len() {
        return Err(ImageError::Truncated);
    }

    Ok(header)
}

/// Parse a payload image, validating its header and checksum.
pub fn parse(image: &[u8]) -> Result<Payload<'_>, ImageError> {
    let header = parse_header(image)?;
    let end = IMAGE_HEADER_SIZE + header.size as usize;
    let data = &image[IMAGE_HEADER_SIZE..end];
    if image_checksum(data) != header.checksum {
        return Err(ImageError::BadChecksum);
    }

    Ok(Payload { header, data })
}

/// Wrap a raw payload binary in an [`ImageHeader`], producing an image [`parse`] accepts.
///
/// The header's `size` and `checksum` are derived from `data`; `magic` and `version` are the
/// current ones.
#[cfg(feature = "alloc")]
pub fn build(data: &[u8], options: ImageOptions) -> Result<Vec<u8>, ImageError> {
    if data.len() > MAX_IMAGE_SIZE as usize {
        return Err(ImageError::SizeTooLarge);
    }

    let header = ImageHeader {
        magic: IMAGE_MAGIC,
        version: IMAGE_VERSION,
        flags: options.flags,
        entry: options.entry,
        load_addr: options.load_addr,
        size: data.len() as u32,
        checksum: image_checksum(data),
    };

    let mut image = Vec::with_capacity(IMAGE_HEADER_SIZE + data.len());
    image.extend_from_slice(&header.to_bytes());
    image.extend_from_slice(data);
    Ok(image)
}

const _: () = {
    use core::mem::{offset_of, size_of};
    assert!(size_of::<ImageHeader>() == IMAGE_HEADER_SIZE);
    assert!(offset_of!(ImageHeader, magic) == offset::MAGIC);
    assert!(offset_of!(ImageHeader, version) == offset::VERSION);
    assert!(offset_of!(ImageHeader, flags) == offset::FLAGS);
    assert!(offset_of!(ImageHeader, entry) == offset::ENTRY);
    assert!(offset_of!(ImageHeader, load_addr) == offset::LOAD_ADDR);
    assert!(offset_of!(ImageHeader, size) == offset::SIZE);
    assert!(offset_of!(ImageHeader, checksum) == offset::CHECKSUM);
};
