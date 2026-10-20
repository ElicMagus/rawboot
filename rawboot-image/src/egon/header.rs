// SPDX-License-Identifier: Unlicense

//! The eGON header layout, shared by the runtime and the host tooling.

/// The `eGON.BT0` magic BROM checks.
pub const EGON_MAGIC: &[u8; 8] = b"eGON.BT0";

/// Seed written into the checksum field before summing (awboot/`mksunxi`).
pub const EGON_CHECKSUM_SEED: u32 = 0x5F0A_6C39;

/// The 48-byte eGON boot header BROM validates.
///
/// This is the single source of truth for the header layout: `rawboot-rt` embeds [`PLACEHOLDER`]
/// in its `.head.head` link section, and the host tooling serialises it with [`to_bytes`]. The
/// fields are private so the placeholder is the only header the runtime can lay down; [`seal`]
/// edits its bytes rather than a second value.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EgonHeader {
    /// ARM branch over the header to `_start` (`0xEA00000A`).
    instruction: u32,
    /// `eGON.BT0`, checked by BROM.
    magic: [u8; 8],
    /// Checksum placeholder; the sealer rewrites it.
    checksum: u32,
    /// Image length placeholder; the sealer rewrites it.
    length: u32,
    /// SPL signature (`0x30`).
    spl_signature: u32,
    /// Boot header version (`0x30303033`).
    boot_header_version: u32,
    /// Return value (`0x00020000`).
    return_value: u32,
    /// Run address (`0x00020000`).
    run_address: u32,
    /// eGON version.
    egon_version: u32,
    /// Platform information.
    platform: [u8; 4],
    /// String pool prefix (`4.0\0`).
    string: [u8; 4],
}

impl EgonHeader {
    /// Size of the header in bytes (`0x30`).
    pub const SIZE: usize = 48;

    /// Offset of `instruction`.
    pub const INSTRUCTION_OFFSET: usize = 0x00;
    /// Offset of `magic`.
    pub const MAGIC_OFFSET: usize = 0x04;
    /// Offset of `checksum`.
    pub const CHECKSUM_OFFSET: usize = 0x0C;
    /// Offset of `length`.
    pub const LENGTH_OFFSET: usize = 0x10;
    /// Offset of `spl_signature`.
    pub const SPL_SIGNATURE_OFFSET: usize = 0x14;
    /// Offset of `boot_header_version`.
    pub const BOOT_HEADER_VERSION_OFFSET: usize = 0x18;
    /// Offset of `return_value`.
    pub const RETURN_VALUE_OFFSET: usize = 0x1C;
    /// Offset of `run_address`.
    pub const RUN_ADDRESS_OFFSET: usize = 0x20;
    /// Offset of `egon_version`.
    pub const EGON_VERSION_OFFSET: usize = 0x24;
    /// Offset of `platform`.
    pub const PLATFORM_OFFSET: usize = 0x28;
    /// Offset of `string`.
    pub const STRING_OFFSET: usize = 0x2C;

    /// The placeholder header the runtime embeds; the sealer rewrites `length` and `checksum`.
    pub const PLACEHOLDER: EgonHeader = EgonHeader {
        instruction: 0xEA00_000A,
        magic: *EGON_MAGIC,
        checksum: 0x1234_5678,
        length: 32768,
        spl_signature: 0x30,
        boot_header_version: 0x3030_3033,
        return_value: 0x0002_0000,
        run_address: 0x0002_0000,
        egon_version: 0,
        platform: [0x00, 0x00, 0x00, 0x00],
        string: [0x34, 0x2e, 0x30, 0x00],
    };

    /// The header's little-endian byte representation.
    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        crate::bytes::write_u32_le(&mut out, Self::INSTRUCTION_OFFSET, self.instruction);
        let mut i = 0;
        while i < 8 {
            out[Self::MAGIC_OFFSET + i] = self.magic[i];
            i += 1;
        }
        crate::bytes::write_u32_le(&mut out, Self::CHECKSUM_OFFSET, self.checksum);
        crate::bytes::write_u32_le(&mut out, Self::LENGTH_OFFSET, self.length);
        crate::bytes::write_u32_le(&mut out, Self::SPL_SIGNATURE_OFFSET, self.spl_signature);
        crate::bytes::write_u32_le(
            &mut out,
            Self::BOOT_HEADER_VERSION_OFFSET,
            self.boot_header_version,
        );
        crate::bytes::write_u32_le(&mut out, Self::RETURN_VALUE_OFFSET, self.return_value);
        crate::bytes::write_u32_le(&mut out, Self::RUN_ADDRESS_OFFSET, self.run_address);
        crate::bytes::write_u32_le(&mut out, Self::EGON_VERSION_OFFSET, self.egon_version);
        let mut i = 0;
        while i < 4 {
            out[Self::PLATFORM_OFFSET + i] = self.platform[i];
            out[Self::STRING_OFFSET + i] = self.string[i];
            i += 1;
        }
        out
    }
}

const _: () = {
    use core::mem::{offset_of, size_of};
    assert!(size_of::<EgonHeader>() == EgonHeader::SIZE);
    assert!(offset_of!(EgonHeader, instruction) == EgonHeader::INSTRUCTION_OFFSET);
    assert!(offset_of!(EgonHeader, magic) == EgonHeader::MAGIC_OFFSET);
    assert!(offset_of!(EgonHeader, checksum) == EgonHeader::CHECKSUM_OFFSET);
    assert!(offset_of!(EgonHeader, length) == EgonHeader::LENGTH_OFFSET);
    assert!(offset_of!(EgonHeader, spl_signature) == EgonHeader::SPL_SIGNATURE_OFFSET);
    assert!(offset_of!(EgonHeader, boot_header_version) == EgonHeader::BOOT_HEADER_VERSION_OFFSET);
    assert!(offset_of!(EgonHeader, return_value) == EgonHeader::RETURN_VALUE_OFFSET);
    assert!(offset_of!(EgonHeader, run_address) == EgonHeader::RUN_ADDRESS_OFFSET);
    assert!(offset_of!(EgonHeader, egon_version) == EgonHeader::EGON_VERSION_OFFSET);
    assert!(offset_of!(EgonHeader, platform) == EgonHeader::PLATFORM_OFFSET);
    assert!(offset_of!(EgonHeader, string) == EgonHeader::STRING_OFFSET);
};
