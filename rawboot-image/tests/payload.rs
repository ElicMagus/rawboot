// SPDX-License-Identifier: CC-BY-4.0

//! Byte-in/byte-out tests for payload header parsing, driven by independently computed vectors.

use rawboot_image::{
    IMAGE_HEADER_SIZE, IMAGE_MAGIC, IMAGE_VERSION, ImageError, ImageHeader, ImageOptions,
    MAX_IMAGE_SIZE, build, image_checksum, parse, parse_header,
};

const BODY: [u8; 8] = [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
const BODY_CHECKSUM: u32 = 0xF3C1_AFDF;

fn image(version: u32, size: u32, checksum: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&IMAGE_MAGIC.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0x4200_0100u32.to_le_bytes());
    bytes.extend_from_slice(&0x4200_0000u32.to_le_bytes());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(body);
    bytes
}

fn valid_image() -> Vec<u8> {
    image(IMAGE_VERSION, BODY.len() as u32, BODY_CHECKSUM, &BODY)
}

#[test]
fn checksum_matches_known_vector() {
    assert_eq!(image_checksum(&BODY), BODY_CHECKSUM);
}

#[test]
fn parses_valid_image_and_exposes_fields_and_data() {
    let bytes = valid_image();
    let parsed = parse(&bytes).unwrap();

    assert_eq!(parsed.header.magic, IMAGE_MAGIC);
    assert_eq!(parsed.header.version, IMAGE_VERSION);
    assert_eq!(parsed.header.flags, 0);
    assert_eq!(parsed.header.entry, 0x4200_0100);
    assert_eq!(parsed.header.load_addr, 0x4200_0000);
    assert_eq!(parsed.header.size, BODY.len() as u32);
    assert_eq!(parsed.header.checksum, BODY_CHECKSUM);
    assert_eq!(parsed.data, BODY);
}

#[test]
fn parse_header_returns_fields_without_verifying_the_checksum() {
    let mut bytes = valid_image();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;

    let header = parse_header(&bytes).unwrap();

    assert_eq!(header.entry, 0x4200_0100);
    assert_eq!(header.load_addr, 0x4200_0000);
    assert_eq!(header.size, BODY.len() as u32);
    assert_eq!(parse(&bytes), Err(ImageError::BadChecksum));
}

#[test]
fn parse_header_rejects_a_buffer_shorter_than_the_declared_payload() {
    let bytes = image(IMAGE_VERSION, 8, BODY_CHECKSUM, &BODY[..4]);

    assert_eq!(parse_header(&bytes), Err(ImageError::Truncated));
}

#[test]
fn parse_header_rejects_bad_magic() {
    let mut bytes = valid_image();
    bytes[0] ^= 0xFF;

    assert_eq!(parse_header(&bytes), Err(ImageError::BadMagic));
}

#[test]
fn rejects_bad_magic() {
    let mut bytes = valid_image();
    bytes[0] ^= 0xFF;

    assert_eq!(parse(&bytes), Err(ImageError::BadMagic));
}

#[test]
fn rejects_unsupported_version() {
    let bytes = image(IMAGE_VERSION + 1, BODY.len() as u32, BODY_CHECKSUM, &BODY);

    assert_eq!(parse(&bytes), Err(ImageError::UnsupportedVersion));
}

#[test]
fn rejects_bad_checksum() {
    let mut bytes = valid_image();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;

    assert_eq!(parse(&bytes), Err(ImageError::BadChecksum));
}

#[test]
fn rejects_buffer_shorter_than_header() {
    let bytes = valid_image();

    assert_eq!(
        parse(&bytes[..IMAGE_HEADER_SIZE - 1]),
        Err(ImageError::Truncated)
    );
}

#[test]
fn rejects_body_truncated_by_the_buffer() {
    let bytes = image(IMAGE_VERSION, 8, BODY_CHECKSUM, &BODY[..4]);

    assert_eq!(parse(&bytes), Err(ImageError::Truncated));
}

#[test]
fn rejects_oversized_declared_length() {
    let bytes = image(IMAGE_VERSION, MAX_IMAGE_SIZE + 1, BODY_CHECKSUM, &BODY);

    assert_eq!(parse(&bytes), Err(ImageError::SizeTooLarge));
}

const LOAD_ADDR: u32 = 0x4300_0000;
const ENTRY: u32 = 0x4300_0100;

fn options() -> ImageOptions {
    ImageOptions {
        load_addr: LOAD_ADDR,
        entry: ENTRY,
        flags: 0,
    }
}

#[test]
fn default_options_load_and_enter_at_the_default_address() {
    let default = ImageOptions::default();

    assert_eq!(default.load_addr, rawboot_image::DEFAULT_LOAD_ADDR);
    assert_eq!(default.entry, rawboot_image::DEFAULT_LOAD_ADDR);
    assert_eq!(default.flags, 0);
}

#[test]
fn build_lays_down_the_expected_header_bytes() {
    let built = build(&BODY, options()).unwrap();

    let mut expected = Vec::new();
    expected.extend_from_slice(&IMAGE_MAGIC.to_le_bytes());
    expected.extend_from_slice(&IMAGE_VERSION.to_le_bytes());
    expected.extend_from_slice(&0u32.to_le_bytes());
    expected.extend_from_slice(&ENTRY.to_le_bytes());
    expected.extend_from_slice(&LOAD_ADDR.to_le_bytes());
    expected.extend_from_slice(&(BODY.len() as u32).to_le_bytes());
    expected.extend_from_slice(&BODY_CHECKSUM.to_le_bytes());

    assert_eq!(&built[..IMAGE_HEADER_SIZE], expected.as_slice());
    assert_eq!(&built[IMAGE_HEADER_SIZE..], &BODY);
}

#[test]
fn build_round_trips_through_parse() {
    let built = build(&BODY, options()).unwrap();

    assert_eq!(
        parse(&built).unwrap(),
        rawboot_image::Payload {
            header: ImageHeader {
                magic: IMAGE_MAGIC,
                version: IMAGE_VERSION,
                flags: 0,
                entry: ENTRY,
                load_addr: LOAD_ADDR,
                size: BODY.len() as u32,
                checksum: BODY_CHECKSUM,
            },
            data: &BODY,
        }
    );
}

#[test]
fn build_rejects_an_oversized_payload() {
    let too_big = vec![0u8; MAX_IMAGE_SIZE as usize + 1];

    assert_eq!(build(&too_big, options()), Err(ImageError::SizeTooLarge));
}
