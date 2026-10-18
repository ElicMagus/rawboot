// SPDX-License-Identifier: Unlicense

#![cfg_attr(not(test), no_std)]

//! Host-testable image tooling for rawboot.
//!
//! Two independent transforms over image bytes, each tested byte-in/byte-out on the host:
//!
//! - [`payload::parse`] reads and validates the fixed header at the front of a payload image.
//!   This is the loader's seam: it pulls the load address, entry point and payload out of the
//!   image the loader is about to hand off. [`payload::build`] is its inverse — the build
//!   tooling's seam — wrapping a raw payload binary in that header.
//! - [`egon::seal`] fills in and pads the Allwinner eGON header of a loader binary so BROM
//!   validates it. This is the build tooling's seam. It takes and returns an owned buffer, so it
//!   lives behind the default `alloc` feature; a `no_std` loader can depend on this crate with
//!   `default-features = false` and still use [`payload::parse`].

#[cfg(feature = "alloc")]
extern crate alloc;

mod checksum;

#[cfg(feature = "alloc")]
pub mod egon;
pub mod payload;

#[cfg(feature = "alloc")]
pub use egon::{
    BROM_PAGE_SIZE, EGON_CHECKSUM_OFFSET, EGON_CHECKSUM_SEED, EGON_HEADER_SIZE, EGON_LENGTH_OFFSET,
    PADDING_BLOCK, PADDING_FLASH, SealError, SealOptions, seal,
};
#[cfg(feature = "alloc")]
pub use payload::build;
pub use payload::{
    DEFAULT_LOAD_ADDR, IMAGE_HEADER_SIZE, IMAGE_MAGIC, IMAGE_VERSION, ImageError, ImageHeader,
    ImageOptions, MAX_IMAGE_SIZE, Payload, image_checksum, parse,
};
