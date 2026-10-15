#![cfg_attr(not(test), no_std)]

//! Host-testable image tooling for rawboot.
//!
//! This crate holds the two pure functions that operate on image bytes:
//!
//! - [`seal`] wraps the loader's raw binary in the Allwinner eGON header layout that BROM
//!   accepts (checksum, padding, optional 2 KiB -> N KiB page scattering).
//! - [`parse`] reads the fixed payload image header, validating magic, version, size and
//!   checksum.
//!
//! Both are `no_std` so the loader can consume [`parse`], and both are exercised by host
//! unit tests with byte-in/byte-out vectors.

extern crate alloc;

pub mod egon;
pub mod payload;

pub use egon::{
    BROM_PAGE_SIZE, EGON_CHECKSUM_SEED, EGON_HEADER_SIZE, PADDING_BLOCK, PADDING_FLASH, SealError,
    SealOptions, seal,
};
pub use payload::{
    DEFAULT_LOAD_ADDR, IMAGE_HEADER_SIZE, IMAGE_MAGIC, IMAGE_VERSION, ImageError, ImageHeader,
    MAX_IMAGE_SIZE, Payload, image_checksum, parse,
};
