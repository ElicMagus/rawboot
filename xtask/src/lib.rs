// SPDX-License-Identifier: Unlicense

//! Host-side checks for the loader image assembled by the `xtask` binary.
//!
//! The eGON header is laid down by `rawboot-rt` and only its length and checksum
//! fields are filled in by `rawboot_image::seal`. If the linker ever scrambles
//! the header again, `seal` would happily checksum a broken image; these checks
//! fail the build instead, so the regression is caught on the host rather than on
//! the board.

use rawboot_image::{ImageError, ImageOptions, build, parse};

pub use rawboot_image::{HeaderError, verify_sealed as verify_sealed_egon};

/// Wrap a flat payload binary in the rawboot image header and check the result parses
/// back, so a contract drift fails the build on the host rather than on the board.
pub fn wrap_payload(raw: &[u8], options: ImageOptions) -> Result<Vec<u8>, ImageError> {
    let image = build(raw, options)?;
    parse(&image)?;
    Ok(image)
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
        rawboot_image::write_u32_le(&mut image, EgonHeader::LENGTH_OFFSET, wrong);
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
