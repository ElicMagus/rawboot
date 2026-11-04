// SPDX-License-Identifier: CC-BY-4.0

//! Little-endian 32-bit helpers shared by the payload-image and eGON tooling.

/// Read a little-endian `u32` at `offset`.
///
/// # Panics
///
/// Panics if `offset + 4` is past the end of `bytes`.
pub const fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Write `value` as a little-endian `u32` at `offset`.
///
/// # Panics
///
/// Panics if `offset + 4` is past the end of `bytes`.
pub const fn write_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    let encoded = value.to_le_bytes();
    bytes[offset] = encoded[0];
    bytes[offset + 1] = encoded[1];
    bytes[offset + 2] = encoded[2];
    bytes[offset + 3] = encoded[3];
}
