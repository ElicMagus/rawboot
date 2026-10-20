// SPDX-License-Identifier: Unlicense

//! Host-side checks for the loader image assembled by the `xtask` binary.
//!
//! The eGON header is laid down by `rawboot-rt` and only its length and checksum
//! fields are filled in by `rawboot_image::seal`. If the linker ever scrambles
//! the header again, `seal` would happily checksum a broken image; these checks
//! fail the build instead, so the regression is caught on the host rather than on
//! the board.

use core::fmt;

use rawboot_image::{
    EGON_CHECKSUM_SEED, EGON_MAGIC, EgonHeader, ImageError, ImageOptions, build, parse,
    read_u32_le, write_u32_le,
};

/// Wrap a flat payload binary in the rawboot image header and check the result parses
/// back, so a contract drift fails the build on the host rather than on the board.
pub fn wrap_payload(raw: &[u8], options: ImageOptions) -> Result<Vec<u8>, ImageError> {
    let image = build(raw, options)?;
    parse(&image)?;
    Ok(image)
}

/// Why an assembled loader image is not a valid eGON image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    /// The image is shorter than the 48-byte eGON header.
    TooShort,
    /// The `eGON.BT0` magic is not at its required offset.
    BadMagic,
    /// The header length field does not match the image length.
    BadLength,
    /// The header checksum does not match the seeded sum of the image.
    BadChecksum,
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            HeaderError::TooShort => "image shorter than the eGON header",
            HeaderError::BadMagic => "missing `eGON.BT0` magic at offset 0x04",
            HeaderError::BadLength => "header length does not match the image length",
            HeaderError::BadChecksum => "eGON checksum mismatch",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for HeaderError {}

/// Check that `image` is a block-sealed eGON image BROM would accept: the magic
/// sits at its required offset, the length field matches the padded image (no
/// page scattering, so the two coincide), and the checksum is the seeded sum over
/// the whole image.
pub fn verify_sealed_egon(image: &[u8]) -> Result<(), HeaderError> {
    if image.len() < EgonHeader::SIZE {
        return Err(HeaderError::TooShort);
    }
    if &image[EgonHeader::MAGIC_OFFSET..EgonHeader::MAGIC_OFFSET + 8] != EGON_MAGIC {
        return Err(HeaderError::BadMagic);
    }
    if read_u32_le(image, EgonHeader::LENGTH_OFFSET) as usize != image.len() {
        return Err(HeaderError::BadLength);
    }

    let mut seeded = image.to_vec();
    write_u32_le(&mut seeded, EgonHeader::CHECKSUM_OFFSET, EGON_CHECKSUM_SEED);
    if read_u32_le(image, EgonHeader::CHECKSUM_OFFSET) != rawboot_image::image_checksum(&seeded) {
        return Err(HeaderError::BadChecksum);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawboot_image::{EgonHeader, IMAGE_MAGIC, IMAGE_VERSION, SealOptions, seal};

    const PAYLOAD_LOAD_ADDR: u32 = 0x4200_0000;

    #[test]
    fn wraps_a_raw_payload_into_a_parseable_image() {
        let raw = [0xDE, 0xAD, 0xBE, 0xEF];

        let image = wrap_payload(&raw, ImageOptions::new(PAYLOAD_LOAD_ADDR)).unwrap();

        let parsed = parse(&image).unwrap();
        assert_eq!(parsed.header.magic, IMAGE_MAGIC);
        assert_eq!(parsed.header.version, IMAGE_VERSION);
        assert_eq!(parsed.header.load_addr, PAYLOAD_LOAD_ADDR);
        assert_eq!(parsed.header.entry, PAYLOAD_LOAD_ADDR);
        assert_eq!(parsed.data, raw);
    }

    /// The 48-byte eGON header `rawboot-rt` lays down, before sealing.
    fn raw_header() -> Vec<u8> {
        EgonHeader::PLACEHOLDER.to_bytes().to_vec()
    }

    fn sealed() -> Vec<u8> {
        seal(&raw_header(), SealOptions::BLOCK).unwrap()
    }

    #[test]
    fn accepts_a_sealed_loader() {
        assert_eq!(verify_sealed_egon(&sealed()), Ok(()));
    }

    #[test]
    fn rejects_an_unsealed_loader() {
        assert_eq!(
            verify_sealed_egon(&raw_header()),
            Err(HeaderError::BadLength)
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut image = sealed();
        image[EgonHeader::MAGIC_OFFSET + 1] ^= 0xFF;
        assert_eq!(verify_sealed_egon(&image), Err(HeaderError::BadMagic));
    }

    #[test]
    fn rejects_a_length_mismatch() {
        let mut image = sealed();
        let wrong = image.len() as u32 - 512;
        write_u32_le(&mut image, EgonHeader::LENGTH_OFFSET, wrong);
        assert_eq!(verify_sealed_egon(&image), Err(HeaderError::BadLength));
    }

    #[test]
    fn rejects_a_corrupted_payload() {
        let mut image = sealed();
        let last = image.len() - 1;
        image[last] ^= 0xFF;
        assert_eq!(verify_sealed_egon(&image), Err(HeaderError::BadChecksum));
    }

    #[test]
    fn rejects_a_truncated_image() {
        assert_eq!(verify_sealed_egon(&[0u8; 8]), Err(HeaderError::TooShort));
    }
}
