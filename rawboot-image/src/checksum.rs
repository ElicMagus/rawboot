// SPDX-License-Identifier: Unlicense

//! Little-endian 32-bit word summing, shared by the payload and eGON checksums.

/// Sum `data` as little-endian 32-bit words with 32-bit wraparound. A trailing partial word is
/// zero-padded.
pub(crate) fn sum_le_u32(data: &[u8]) -> u32 {
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
