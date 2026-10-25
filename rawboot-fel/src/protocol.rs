// SPDX-License-Identifier: Unlicense

//! The Allwinner FEL wire protocol as implemented by the BROM.
//!
//! FEL runs over bulk endpoints only. Every exchange is wrapped in a 32-byte
//! `AWUC` header that names the direction and payload length; the device closes
//! each write/read with a 13-byte `AWUS` status word.
//!
//! A FEL command itself is a 16-byte little-endian request
//! (`command`, `address`, `length`, padding) sent as the payload of an `AWUC`
//! write. Reads and writes are then carried by further `AWUC` transfers, and
//! every command ends with an 8-byte status read.
//!
//! | command | code    | payload              |
//! | ------- | ------- | -------------------- |
//! | version | `0x001` | 32-byte version reply |
//! | write   | `0x101` | caller's bytes       |
//! | exec    | `0x102` | none                 |
//! | read    | `0x103` | requested bytes      |
//!
//! The BROM accepts at most 64 KiB per read or write, so the device layer
//! splits larger accesses into chunks of that size.

use crate::error::ProtocolError;

/// The largest payload the BROM accepts in one FEL read or write.
pub const MAX_CHUNK_SIZE: usize = 64 * 1024;

/// `AWUC` request that carries bytes to the device.
pub(crate) const USB_WRITE: u16 = 0x12;
/// `AWUC` request that fetches bytes from the device.
pub(crate) const USB_READ: u16 = 0x11;

/// FEL command returning the BROM's version block.
pub(crate) const FEL_VERSION: u32 = 0x001;
/// FEL command writing caller bytes to memory.
pub(crate) const FEL_WRITE: u32 = 0x101;
/// FEL command jumping to an address.
pub(crate) const FEL_EXEC: u32 = 0x102;
/// FEL command reading device memory.
pub(crate) const FEL_READ: u32 = 0x103;

/// Size of the `AWUC` request header.
pub(crate) const USB_HEADER_SIZE: usize = 32;
/// Size of the `AWUS` response word.
pub(crate) const USB_RESPONSE_SIZE: usize = 13;
/// Size of a FEL command request.
pub(crate) const FEL_REQUEST_SIZE: usize = 16;
/// Size of the status read that closes every FEL command.
pub(crate) const FEL_STATUS_SIZE: usize = 8;
/// Size of the version reply.
pub(crate) const VERSION_SIZE: usize = 32;

const USB_SIGNATURE: &[u8; 4] = b"AWUC";
const USB_RESPONSE_SIGNATURE: &[u8; 4] = b"AWUS";
// Opaque word the BROM expects in every AWUC header; the name follows the
// upstream sunxi-tools/xfel field (`unknown1`).
const USB_HEADER_UNKNOWN: u32 = 0x0c00_0000;

/// Chip ID reported by the FEL version reply for the D1/D1s/R528/T113-S3 family.
pub const CHIP_ID_T113: u16 = 0x1859;

/// ARM branch word at address zero on a T113-S3 running the FEL BROM.
///
/// The chip ID alone cannot separate the T113-S3 from the RISC-V D1/D1s/R528
/// that share it, so identification also samples the reset vector.
pub const T113_VECTOR: u32 = 0xea00_0019;

/// Build the 32-byte `AWUC` header for one bulk transfer.
pub(crate) fn usb_request(request: u16, length: u32) -> [u8; USB_HEADER_SIZE] {
    let mut frame = [0u8; USB_HEADER_SIZE];
    frame[..USB_SIGNATURE.len()].copy_from_slice(USB_SIGNATURE);
    frame[8..12].copy_from_slice(&length.to_le_bytes());
    frame[12..16].copy_from_slice(&USB_HEADER_UNKNOWN.to_le_bytes());
    frame[16..18].copy_from_slice(&request.to_le_bytes());
    frame[18..22].copy_from_slice(&length.to_le_bytes());
    frame
}

/// Build a 16-byte FEL command request.
pub(crate) fn fel_request(request: u32, address: u32, length: u32) -> [u8; FEL_REQUEST_SIZE] {
    let mut frame = [0u8; FEL_REQUEST_SIZE];
    frame[0..4].copy_from_slice(&request.to_le_bytes());
    frame[4..8].copy_from_slice(&address.to_le_bytes());
    frame[8..12].copy_from_slice(&length.to_le_bytes());
    frame
}

/// Whether a response begins with the `AWUS` signature.
pub(crate) fn is_awus(response: &[u8]) -> bool {
    response.starts_with(USB_RESPONSE_SIGNATURE)
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// The 32-byte version block returned by [`FEL_VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FelVersion {
    /// BROM signature bytes, normally an ASCII egon image signature.
    pub signature: [u8; 8],
    /// Raw SoC ID word; the chip ID is its second byte, see [`Self::chip_id`].
    pub soc_id: u32,
    /// Firmware word reported by the BROM.
    pub firmware: u32,
    /// FEL protocol version.
    pub protocol: u16,
    /// Flags byte reported by the BROM.
    pub dflag: u8,
    /// Length byte reported by the BROM.
    pub dlength: u8,
    /// BROM scratch address.
    pub scratchpad: u32,
}

impl FelVersion {
    /// Decode the 32-byte reply.
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < VERSION_SIZE {
            return Err(ProtocolError::ShortVersion {
                length: bytes.len(),
            });
        }

        let mut signature = [0u8; 8];
        signature.copy_from_slice(&bytes[..8]);

        Ok(Self {
            signature,
            soc_id: le_u32(bytes, 8),
            firmware: le_u32(bytes, 12),
            protocol: le_u16(bytes, 16),
            dflag: bytes[18],
            dlength: bytes[19],
            scratchpad: le_u32(bytes, 20),
        })
    }

    /// The 16-bit chip ID embedded in the raw SoC ID word.
    pub fn chip_id(&self) -> u16 {
        ((self.soc_id >> 8) & 0xffff) as u16
    }
}

/// A supported Allwinner SoC family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Soc {
    /// Allwinner T113-S3, the SoC this bootloader targets.
    T113,
}

impl Soc {
    /// Map a FEL chip ID to a supported SoC.
    ///
    /// `0x1859` is shared by the D1/D1s/R528/T113-S3; the caller must also
    /// check the reset vector before calling the part a T113-S3.
    pub fn from_chip_id(chip_id: u16) -> Option<Self> {
        (chip_id == CHIP_ID_T113).then_some(Self::T113)
    }

    /// Human-readable part name.
    pub fn name(self) -> &'static str {
        match self {
            Self::T113 => "T113-S3",
        }
    }
}

#[cfg(test)]
pub(crate) fn version_reply(soc_id: u32) -> [u8; VERSION_SIZE] {
    let mut bytes = [0u8; VERSION_SIZE];
    bytes[..4].copy_from_slice(b"eGON");
    bytes[8..12].copy_from_slice(&soc_id.to_le_bytes());
    bytes[12..16].copy_from_slice(&0x0000_0001u32.to_le_bytes());
    bytes[16..18].copy_from_slice(&1u16.to_le_bytes());
    bytes[18] = 0x44;
    bytes[19] = 0x08;
    bytes[20..24].copy_from_slice(&0x0000_7e00u32.to_le_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awuc_header_matches_the_brom_layout() {
        let frame = usb_request(USB_WRITE, 0x1234);

        assert_eq!(frame.len(), 32);
        assert_eq!(&frame[..8], b"AWUC\0\0\0\0");
        assert_eq!(le_u32(&frame, 8), 0x1234);
        assert_eq!(le_u32(&frame, 12), 0x0c00_0000);
        assert_eq!(le_u16(&frame, 16), 0x12);
        assert_eq!(le_u32(&frame, 18), 0x1234);
        assert!(frame[22..].iter().all(|&b| b == 0), "padding is zero");
    }

    #[test]
    fn fel_request_is_little_endian_and_zero_padded() {
        let frame = fel_request(FEL_WRITE, 0x0003_9000, 0x10);

        assert_eq!(frame.len(), 16);
        assert_eq!(le_u32(&frame, 0), 0x101);
        assert_eq!(le_u32(&frame, 4), 0x0003_9000);
        assert_eq!(le_u32(&frame, 8), 0x10);
        assert_eq!(le_u32(&frame, 12), 0);
    }

    #[test]
    fn awus_detection_checks_only_the_signature() {
        let mut response = [0u8; USB_RESPONSE_SIZE];
        response[..4].copy_from_slice(b"AWUS");
        assert!(is_awus(&response));

        response[0] = b'X';
        assert!(!is_awus(&response));
    }

    #[test]
    fn decodes_the_version_reply() {
        let version = FelVersion::parse(&version_reply(0x0018_5900)).unwrap();

        assert_eq!(&version.signature[..4], b"eGON");
        assert_eq!(version.soc_id, 0x0018_5900);
        assert_eq!(version.chip_id(), CHIP_ID_T113);
        assert_eq!(version.firmware, 1);
        assert_eq!(version.protocol, 1);
        assert_eq!(version.dflag, 0x44);
        assert_eq!(version.dlength, 0x08);
        assert_eq!(version.scratchpad, 0x7e00);
    }

    #[test]
    fn rejects_a_short_version_reply() {
        assert_eq!(
            FelVersion::parse(&[0u8; VERSION_SIZE - 1]),
            Err(ProtocolError::ShortVersion {
                length: VERSION_SIZE - 1
            })
        );
    }

    #[test]
    fn recognises_the_t113_chip_id() {
        assert_eq!(Soc::from_chip_id(CHIP_ID_T113), Some(Soc::T113));
        assert_eq!(Soc::from_chip_id(0x1623), None);
        assert_eq!(Soc::T113.name(), "T113-S3");
    }
}
