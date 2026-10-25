// SPDX-License-Identifier: Unlicense

//! Allwinner eGON boot-image layout and sealing.
//!
//! BROM validates an eGON image before running it: it checks that a 32-bit checksum over the
//! padded image matches the value in the header, with the checksum field seeded to
//! [`EGON_CHECKSUM_SEED`] before summing. [`seal`] reproduces the `mksunxi` transform used by
//! awboot: it pads the image to a block or flash boundary, overwrites the header's length field
//! with the padded length, computes the seeded checksum, and — for BROM page sizes larger than
//! 2 KiB — scatters each 2 KiB logical page into a zero-filled page slot.
//!
//! The input is the loader binary *with* its eGON header already laid down by [`EgonHeader`] in
//! `rawboot-rt`; only the length and checksum fields are modified.
//!
//! This differs from `mksunxi` on purpose: `mksunxi` sums `ALIGN(header.length, padding)` words,
//! while [`seal`] derives the length from the actual input size. The header field is an output
//! here, so a stale placeholder cannot under- or over-read the buffer, and BROM still accepts the
//! image because it sums exactly the length the header ends up declaring. This is a deliberate
//! divergence from upstream, accepted in ADR-0009 against the parity rule of ADR-0007.
//!
//! [`EgonHeader`] and its constants are available without `alloc`; [`seal`] and its options are
//! behind the `alloc` feature.

mod header;
#[cfg(feature = "alloc")]
mod seal;
mod verify;

pub use header::{EGON_CHECKSUM_SEED, EGON_MAGIC, EgonHeader};
#[cfg(feature = "alloc")]
pub use seal::{
    BROM_PAGE_SIZE, PADDING_BLOCK, PADDING_FLASH, Padding, PageSize, SealError, SealOptions, seal,
};
pub use verify::{HeaderError, verify_sealed};
