// SPDX-License-Identifier: CC-BY-4.0

//! Checking that an assembled loader image is a block-sealed eGON image.

use core::fmt;

use crate::bytes::read_u32_le;
use crate::checksum::image_checksum;

use super::header::{EGON_CHECKSUM_SEED, EGON_MAGIC, EgonHeader};

/// Why an assembled loader image is not a valid eGON image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    /// The image is shorter than the 48-byte eGON header.
    TooShort,
    /// The `eGON.BT0` magic is not at its required offset.
    BadMagic,
    /// The header length field does not match the image length.
    BadLength,
    /// The header checksum does not match the seeded sum of the image.
    BadChecksum,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            VerifyError::TooShort => "image shorter than the eGON header",
            VerifyError::BadMagic => "missing `eGON.BT0` magic at offset 0x04",
            VerifyError::BadLength => "header length does not match the image length",
            VerifyError::BadChecksum => "eGON checksum mismatch",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for VerifyError {}

/// Check that `image` is a block-sealed eGON image BROM would accept: the magic
/// sits at its required offset, the length field matches the padded image (no
/// page scattering, so the two coincide), and the checksum is the seeded sum over
/// the whole image.
pub fn verify_sealed(image: &[u8]) -> Result<(), VerifyError> {
    if image.len() < EgonHeader::SIZE {
        return Err(VerifyError::TooShort);
    }
    if &image[EgonHeader::MAGIC_OFFSET..EgonHeader::MAGIC_OFFSET + 8] != EGON_MAGIC {
        return Err(VerifyError::BadMagic);
    }
    if read_u32_le(image, EgonHeader::LENGTH_OFFSET) as usize != image.len() {
        return Err(VerifyError::BadLength);
    }

    let stored = read_u32_le(image, EgonHeader::CHECKSUM_OFFSET);
    let seeded = image_checksum(image)
        .wrapping_sub(stored)
        .wrapping_add(EGON_CHECKSUM_SEED);
    if stored != seeded {
        return Err(VerifyError::BadChecksum);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytes::write_u32_le;
    use crate::egon::{SealOptions, seal};

    fn sealed() -> Vec<u8> {
        seal(&EgonHeader::PLACEHOLDER.to_bytes(), SealOptions::BLOCK).unwrap()
    }

    #[test]
    fn accepts_a_sealed_loader() {
        assert_eq!(verify_sealed(&sealed()), Ok(()));
    }

    #[test]
    fn rejects_an_unsealed_loader() {
        assert_eq!(
            verify_sealed(&EgonHeader::PLACEHOLDER.to_bytes()),
            Err(VerifyError::BadLength)
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut image = sealed();
        image[EgonHeader::MAGIC_OFFSET + 1] ^= 0xFF;
        assert_eq!(verify_sealed(&image), Err(VerifyError::BadMagic));
    }

    #[test]
    fn rejects_a_truncated_image() {
        assert_eq!(verify_sealed(&[0u8; 8]), Err(VerifyError::TooShort));
    }

    #[test]
    fn rejects_a_corrupted_payload() {
        let mut image = sealed();
        let last = image.len() - 1;
        image[last] ^= 0xFF;
        assert_eq!(verify_sealed(&image), Err(VerifyError::BadChecksum));
    }

    #[test]
    fn rejects_a_length_mismatch() {
        let mut image = sealed();
        let wrong = image.len() as u32 - 512;
        write_u32_le(&mut image, EgonHeader::LENGTH_OFFSET, wrong);
        assert_eq!(verify_sealed(&image), Err(VerifyError::BadLength));
    }
}
