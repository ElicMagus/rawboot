// SPDX-License-Identifier: CC-BY-4.0

//! Little-endian 32-bit word summing for the payload and eGON checksums.

/// The image checksum: the wrapping sum of `data` as little-endian 32-bit words, with a trailing
/// partial word zero-padded.
pub fn image_checksum(data: &[u8]) -> u32 {
    let (words, rest) = data.as_chunks::<4>();
    let mut sum = 0u32;
    for word in words {
        sum = sum.wrapping_add(u32::from_le_bytes(*word));
    }
    if !rest.is_empty() {
        let mut word = [0u8; 4];
        word[..rest.len()].copy_from_slice(rest);
        sum = sum.wrapping_add(u32::from_le_bytes(word));
    }
    sum
}
