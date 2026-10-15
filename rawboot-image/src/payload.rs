//! Parsing of the rawboot payload image header.
//!
//! The payload ships as a raw binary behind a fixed 28-byte [`ImageHeader`]. `size` counts the
//! payload bytes that follow the header, and `checksum` is the wrapping sum of those bytes as
//! little-endian 32-bit words (a trailing partial word is zero-padded).

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

/// Parse a payload image, validating its header and checksum.
pub fn parse(image: &[u8]) -> Result<Payload<'_>, ImageError> {
    if image.len() < IMAGE_HEADER_SIZE {
        return Err(ImageError::Truncated);
    }

    let header = ImageHeader {
        magic: read_u32(image, 0),
        version: read_u32(image, 4),
        flags: read_u32(image, 8),
        entry: read_u32(image, 12),
        load_addr: read_u32(image, 16),
        size: read_u32(image, 20),
        checksum: read_u32(image, 24),
    };

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

    let data = &image[IMAGE_HEADER_SIZE..end];
    if image_checksum(data) != header.checksum {
        return Err(ImageError::BadChecksum);
    }

    Ok(Payload { header, data })
}

/// The payload checksum: the wrapping sum of `data` as little-endian 32-bit words, with a
/// trailing partial word zero-padded.
pub fn image_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_le_bytes(word));
    }
    sum
}

const fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

const _: () = {
    use core::mem::{offset_of, size_of};
    assert!(size_of::<ImageHeader>() == IMAGE_HEADER_SIZE);
    assert!(offset_of!(ImageHeader, magic) == 0);
    assert!(offset_of!(ImageHeader, version) == 4);
    assert!(offset_of!(ImageHeader, flags) == 8);
    assert!(offset_of!(ImageHeader, entry) == 12);
    assert!(offset_of!(ImageHeader, load_addr) == 16);
    assert!(offset_of!(ImageHeader, size) == 20);
    assert!(offset_of!(ImageHeader, checksum) == 24);
};
