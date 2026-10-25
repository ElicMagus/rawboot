// SPDX-License-Identifier: Unlicense

#![cfg_attr(not(test), no_std)]

//! Host-testable image and boot-contract tooling for rawboot.
//!
//! Two independent transforms over image bytes, each tested byte-in/byte-out on the host:
//!
//! - [`payload::parse`] reads and validates the fixed header at the front of a payload image,
//!   plus the payload checksum. [`payload::parse_header`] is the loader's first step: it reads
//!   the header alone, so the loader can place the payload before verifying its checksum at the
//!   destination. [`payload::build`] is their inverse — the build tooling's seam — wrapping a
//!   raw payload binary in that header.
//! - [`egon::seal`] fills in and pads the Allwinner eGON header of a loader binary so BROM
//!   validates it. This is the build tooling's seam. It takes and returns an owned buffer, so it
//!   lives behind the default `alloc` feature; a `no_std` loader can depend on this crate with
//!   `default-features = false` and still use [`payload::parse`]. The [`egon::EgonHeader`] layout
//!   itself is shared with the runtime and needs no allocator. [`egon::verify_sealed`] is the
//!   inverse check, used before an assembled image is accepted for the board.
//! - [`sd::build_sd_image`] wraps a sealed SD/SPI loader in the boot prefix: an MBR pointing at an
//!   existing FAT partition plus the loader at sector 16. It is the build tooling's seam for
//!   putting the loader on a card without touching the partition.
//! - [`fel::FelMailbox`] is the host-written FEL request that tells the loader where the payload
//!   image and its argument string are staged in SRAM.
//! - [`flash`] owns the fixed DRAM addresses and the checksummed [`flash::FlashJob`] and
//!   [`flash::FlashStatus`] blocks the host and the FEL flasher exchange (ADR-0013).

#[cfg(feature = "alloc")]
extern crate alloc;

mod bytes;
mod checksum;

pub mod egon;
pub mod fel;
pub mod flash;
pub mod payload;
#[cfg(feature = "alloc")]
pub mod sd;

pub use bytes::{read_u32_le, write_u32_le};
pub use checksum::image_checksum;
#[cfg(feature = "alloc")]
pub use egon::{
    BROM_PAGE_SIZE, PADDING_BLOCK, PADDING_FLASH, Padding, PageSize, SealError, SealOptions, seal,
};
pub use egon::{EGON_CHECKSUM_SEED, EGON_MAGIC, EgonHeader, VerifyError, verify_sealed};
pub use fel::{
    FEL_ARGS_BASE, FEL_ARGS_MAX_SIZE, FEL_MAILBOX_BASE, FEL_MAILBOX_MAGIC, FEL_MAILBOX_WORDS,
    FEL_PAYLOAD_BASE, FEL_PAYLOAD_MAX_SIZE, FelMailbox,
};
pub use flash::{
    DDR_RESULT_ADDR, DDR_RESULT_MAGIC, DDR_RESULT_SIZE, DDR_STUB_LOAD_ADDR, DDR_STUB_MAX_SIZE,
    DRAM_TEST_ADDR, DRAM_TEST_SIZE, DdrResult, FLASH_COMMAND_NOOP, FLASH_CONTRACT_VERSION,
    FLASH_JOB_ADDR, FLASH_JOB_MAGIC, FLASH_JOB_SIZE, FLASH_RESULT_BAD_JOB, FLASH_RESULT_OK,
    FLASH_RESULT_UNKNOWN_COMMAND, FLASH_STATUS_ADDR, FLASH_STATUS_MAGIC, FLASH_STATUS_SIZE,
    FLASH_STEP_DONE, FLASH_STEP_STARTED, FLASHER_LOAD_ADDR, FLASHER_MAX_SIZE, FLASHER_STACK_SIZE,
    FLASHER_STACK_TOP, FLASHER_WINDOW_SIZE, FlashContractError, FlashJob, FlashStatus, result_name,
    step_name,
};
#[cfg(feature = "alloc")]
pub use payload::build;
pub use payload::{
    DEFAULT_LOAD_ADDR, IMAGE_HEADER_SIZE, IMAGE_MAGIC, IMAGE_VERSION, ImageError, ImageHeader,
    ImageOptions, MAX_IMAGE_SIZE, Payload, parse, parse_header,
};
#[cfg(feature = "alloc")]
pub use sd::{
    DEFAULT_START_LBA, DISK_SIGNATURE_OFFSET, MBR_SIGNATURE, MBR_SIGNATURE_OFFSET,
    PARTITION_ENTRY_SIZE, PARTITION_TABLE_OFFSET, PARTITION_TYPE_FAT32, SD_LOADER_OFFSET,
    SECTOR_SIZE, SdImageError, SdImageOptions, build_sd_image,
};
