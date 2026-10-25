// SPDX-License-Identifier: Unlicense

//! Host-side checks for the payload image assembled by the `xtask` binary.
//!
//! eGON verification lives in `rawboot-image`, next to the sealer it checks.

use rawboot_image::{ImageError, ImageOptions, build, parse};

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
    use rawboot_image::{IMAGE_MAGIC, IMAGE_VERSION};

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
}
