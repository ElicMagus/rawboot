// SPDX-License-Identifier: Unlicense

//! The in-DRAM contract between the host, the DDR-init stub and the FEL flasher.
//!
//! The host first runs a small DDR-init stub that returns to the BROM. It then
//! writes a [`FlashJob`] and the flasher to fixed DRAM addresses, executes the
//! flasher, and reads a [`FlashStatus`] after the flasher returns. This module
//! owns the addresses and the versioned, checksummed blocks both sides use, so
//! the host tests and the on-device code cannot drift apart (ADR-0013).
//!
//! All blocks are little-endian and laid out field by field, not by writing the
//! Rust struct directly: the contract is the byte layout, and the compile-time
//! assertions below keep the struct offsets honest.

use crate::bytes::{read_u32_le, write_u32_le};
use crate::checksum::image_checksum;
use crate::fel::FEL_MAILBOX_BASE;

/// First byte of the SRAM window the host loads the DDR-init stub into.
///
/// It reuses the loader's FEL origin: the stub runs first, returns to the
/// BROM, and the host then loads the flasher into DRAM, so the two never need
/// to coexist.
pub const DDR_STUB_LOAD_ADDR: u32 = 0x0002_8000;
/// Largest DDR-init stub the host will load.
///
/// The result block sits directly above it. The window is smaller than the
/// loader's 64 KiB region and ends well inside the area the external `xfel`
/// DDR payload uses, so it stays clear of the BROM's own stack.
pub const DDR_STUB_MAX_SIZE: u32 = 20 * 1024;
/// Where the stub leaves its result: magic first, then the detected size in MiB.
pub const DDR_RESULT_ADDR: u32 = DDR_STUB_LOAD_ADDR + DDR_STUB_MAX_SIZE;
/// `RBDD` in little-endian order: the DDR-init stub ran and wrote a result.
pub const DDR_RESULT_MAGIC: u32 = u32::from_le_bytes(*b"RBDD");
/// Number of bytes in a [`DdrResult`].
pub const DDR_RESULT_SIZE: usize = 8;

/// What the DDR-init stub leaves at [`DDR_RESULT_ADDR`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DdrResult {
    /// [`DDR_RESULT_MAGIC`].
    pub magic: u32,
    /// Detected DRAM size in MiB; zero means the driver failed.
    pub size_mib: u32,
}

impl DdrResult {
    /// A result for a successful DRAM bring-up.
    pub const fn new(size_mib: u32) -> Self {
        Self {
            magic: DDR_RESULT_MAGIC,
            size_mib,
        }
    }

    /// The result's little-endian byte representation, as left in SRAM.
    pub const fn to_bytes(self) -> [u8; DDR_RESULT_SIZE] {
        let mut out = [0u8; DDR_RESULT_SIZE];
        write_u32_le(&mut out, 0, self.magic);
        write_u32_le(&mut out, 4, self.size_mib);
        out
    }

    /// Decode a result, or return `None` when the stub did not write its magic.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < DDR_RESULT_SIZE || read_u32_le(bytes, 0) != DDR_RESULT_MAGIC {
            return None;
        }
        Some(Self {
            magic: DDR_RESULT_MAGIC,
            size_mib: read_u32_le(bytes, 4),
        })
    }
}

/// Start of the in-DRAM job block.
pub const FLASH_JOB_ADDR: u32 = 0x4000_0000;
/// Start of the in-DRAM status block.
pub const FLASH_STATUS_ADDR: u32 = 0x4000_0100;
/// Scratch window for the host's DRAM read-back check after the stub.
pub const DRAM_TEST_ADDR: u32 = 0x4000_2000;
/// Length of the DRAM read-back check.
pub const DRAM_TEST_SIZE: u32 = 0x1000;

/// Where the host places the flasher in DRAM.
pub const FLASHER_LOAD_ADDR: u32 = 0x4001_0000;
/// Total DRAM window reserved for the flasher and its stack.
pub const FLASHER_WINDOW_SIZE: u32 = 0x0004_0000;
/// Stack reserved at the top of the flasher window.
pub const FLASHER_STACK_SIZE: u32 = 0x4000;
/// Largest flasher image; the rest of the window is its stack.
pub const FLASHER_MAX_SIZE: u32 = FLASHER_WINDOW_SIZE - FLASHER_STACK_SIZE;
/// Top of the flasher's DRAM stack; the flasher switches to it on entry.
pub const FLASHER_STACK_TOP: u32 = FLASHER_LOAD_ADDR + FLASHER_WINDOW_SIZE;

/// Base of the DRAM the DDR-init stub makes usable.
pub const FLASH_DRAM_BASE: u32 = 0x4000_0000;
/// Where the host places a [`FlashPlan`] for an SD write.
pub const FLASH_PLAN_ADDR: u32 = 0x4000_0200;
/// First byte of the staged payload image.
pub const FLASH_PAYLOAD_ADDR: u32 = 0x4010_0000;
/// Largest payload image the host may stage.
pub const FLASH_PAYLOAD_MAX_SIZE: u32 = 8 * 1024 * 1024;
/// First byte of the staged optional `boot.cfg` contents.
pub const FLASH_ARGS_ADDR: u32 = FLASH_PAYLOAD_ADDR + FLASH_PAYLOAD_MAX_SIZE;
/// Largest `boot.cfg` contents; the loader adds the NUL terminator itself.
pub const FLASH_ARGS_MAX_SIZE: u32 = 4095;
/// First byte of the staged sealed SD/SPI loader.
pub const FLASH_LOADER_ADDR: u32 = 0x4090_1000;
/// Largest loader image; the SD/SPI loader must fit the loader's SRAM budget.
pub const FLASH_LOADER_MAX_SIZE: u32 = 64 * 1024;
/// One past the last staged byte; the host checks it against detected DRAM.
pub const FLASH_STAGE_END: u32 = FLASH_LOADER_ADDR + FLASH_LOADER_MAX_SIZE;
/// DRAM the DDR-init stub must report for the whole staging window.
pub const FLASH_STAGE_REQUIRED_SIZE: u32 = FLASH_STAGE_END - FLASH_DRAM_BASE;

/// SD block size used by the MBR and by the loader placement.
pub const FLASH_SECTOR_SIZE: u64 = 512;
/// First SD sector BROM reads the boot image from.
pub const FLASH_LOADER_START_LBA: u64 = 16;

/// Why an MBR/FAT layout cannot safely receive the loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashLayoutError {
    /// The FAT partition starts at or inside the loader's sector.
    PartitionTooEarly,
    /// The loader would reach into the FAT partition.
    LoaderOverlapsPartition,
    /// The partition extends past the card.
    PartitionBeyondCard,
    /// A sector or byte address overflowed.
    Overflow,
}

impl core::fmt::Display for FlashLayoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PartitionTooEarly => f.write_str("FAT partition starts at or before sector 16"),
            Self::LoaderOverlapsPartition => {
                f.write_str("loader at sector 16 reaches into the FAT partition")
            }
            Self::PartitionBeyondCard => f.write_str("FAT partition extends past the card"),
            Self::Overflow => f.write_str("card layout overflows the sector address width"),
        }
    }
}

impl core::error::Error for FlashLayoutError {}

/// Check that `loader_len` bytes at sector 16 fit between the MBR and a FAT
/// partition that itself lies inside the card.
pub fn check_sd_layout(
    partition_start_lba: u64,
    partition_len_bytes: u64,
    loader_len: u32,
    card_bytes: u64,
) -> Result<(), FlashLayoutError> {
    let loader_start = FLASH_LOADER_START_LBA
        .checked_mul(FLASH_SECTOR_SIZE)
        .ok_or(FlashLayoutError::Overflow)?;
    let partition_start = partition_start_lba
        .checked_mul(FLASH_SECTOR_SIZE)
        .ok_or(FlashLayoutError::Overflow)?;
    let loader_end = loader_start
        .checked_add(loader_len as u64)
        .ok_or(FlashLayoutError::Overflow)?;
    let partition_end = partition_start
        .checked_add(partition_len_bytes)
        .ok_or(FlashLayoutError::Overflow)?;

    if partition_start_lba <= FLASH_LOADER_START_LBA {
        return Err(FlashLayoutError::PartitionTooEarly);
    }
    if loader_end > partition_start {
        return Err(FlashLayoutError::LoaderOverlapsPartition);
    }
    if partition_end > card_bytes {
        return Err(FlashLayoutError::PartitionBeyondCard);
    }
    Ok(())
}

/// The block layout version shared by jobs, statuses and plans.
pub const FLASH_CONTRACT_VERSION: u32 = 1;

/// `RBFJ` in little-endian order: a host-written flash job.
pub const FLASH_JOB_MAGIC: u32 = u32::from_le_bytes(*b"RBFJ");
/// `RBFS` in little-endian order: a flasher-written status.
pub const FLASH_STATUS_MAGIC: u32 = u32::from_le_bytes(*b"RBFS");
/// `RBFP` in little-endian order: a host-written plan for an SD write.
pub const FLASH_PLAN_MAGIC: u32 = u32::from_le_bytes(*b"RBFP");

/// There is no work to do beyond proving the path; used by the #41 spike.
pub const FLASH_COMMAND_NOOP: u32 = 0;
/// Write the staged payload files and loader to the SD card.
pub const FLASH_COMMAND_FLASH_SD: u32 = 1;
/// Write a known block to the loader sector and read it back.
///
/// This is a diagnostic for the SD block-write path: it needs no MBR, FAT or
/// staged images, and it never touches sector 0.
pub const FLASH_COMMAND_RAW_WRITE_TEST: u32 = 2;
/// The flasher finished successfully.
pub const FLASH_RESULT_OK: u32 = 0;
/// The flasher could not parse the job block.
pub const FLASH_RESULT_BAD_JOB: u32 = 1;
/// The job names a command this flasher does not implement.
pub const FLASH_RESULT_UNKNOWN_COMMAND: u32 = 2;
/// The plan block or one of the staged images is invalid.
pub const FLASH_RESULT_BAD_PLAN: u32 = 3;
/// The SD/MMC card did not initialise.
pub const FLASH_RESULT_SD_OFFLINE: u32 = 4;
/// The card's first sector does not carry an MBR.
pub const FLASH_RESULT_NO_MBR: u32 = 5;
/// The MBR has no FAT partition the loader could mount.
pub const FLASH_RESULT_NO_FAT: u32 = 6;
/// The loader would overlap the MBR/partition, or the partition is unsafe.
pub const FLASH_RESULT_UNSAFE_LAYOUT: u32 = 7;
/// The card is too small for its own partition or the loader.
pub const FLASH_RESULT_CARD_TOO_SMALL: u32 = 8;
/// A card or filesystem write failed.
pub const FLASH_RESULT_WRITE_FAILED: u32 = 9;
/// A read-back comparison failed.
pub const FLASH_RESULT_VERIFY_FAILED: u32 = 10;
/// The MBR changed during the write, which must never happen.
pub const FLASH_RESULT_MBR_CHANGED: u32 = 11;
/// A card read failed.
pub const FLASH_RESULT_READ_FAILED: u32 = 12;
/// Creating a FAT file failed.
pub const FLASH_RESULT_FILE_CREATE: u32 = 13;
/// Truncating a FAT file failed.
pub const FLASH_RESULT_FILE_TRUNCATE: u32 = 14;
/// Writing FAT file data failed.
pub const FLASH_RESULT_FILE_WRITE: u32 = 15;
/// Flushing FAT file data failed.
pub const FLASH_RESULT_FILE_FLUSH: u32 = 16;
/// The flasher has written its final status.
///
/// Step 1 used to mean "started", but the flasher never wrote it: the host reads
/// the status only after the flasher returns, so a running status is not
/// observable. Step values stay fixed so the on-wire layout does not drift.
pub const FLASH_STEP_DONE: u32 = 2;
/// The flasher is validating the plan and staged images.
pub const FLASH_STEP_PLAN: u32 = 3;
/// The flasher is initialising the SD/MMC card.
pub const FLASH_STEP_SD_INIT: u32 = 4;
/// The flasher is checking the MBR, FAT partition and loader placement.
pub const FLASH_STEP_LAYOUT: u32 = 5;
/// The flasher is writing `payload.bin` to the FAT root.
pub const FLASH_STEP_WRITE_PAYLOAD: u32 = 6;
/// The flasher is writing `boot.cfg` to the FAT root.
pub const FLASH_STEP_WRITE_ARGS: u32 = 7;
/// The flasher is reading the FAT files back.
pub const FLASH_STEP_VERIFY_FAT: u32 = 8;
/// The flasher is writing the loader to sector 16.
pub const FLASH_STEP_WRITE_LOADER: u32 = 9;
/// The flasher is reading the loader and MBR back.
pub const FLASH_STEP_VERIFY_LOADER: u32 = 10;
/// The flasher is reading the MBR from the card.
pub const FLASH_STEP_READ_MBR: u32 = 11;
/// The flasher is writing the diagnostic block to sector 16.
pub const FLASH_STEP_RAW_WRITE: u32 = 12;
/// The flasher is reading the diagnostic block back.
pub const FLASH_STEP_RAW_VERIFY: u32 = 13;

/// Bytes in the header every versioned flash block shares.
const BLOCK_HEADER_SIZE: usize = 16;

mod header_offset {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 4;
    pub const SIZE: usize = 8;
    pub const CHECKSUM: usize = 12;
}

mod job_offset {
    pub use super::header_offset::{CHECKSUM, MAGIC};

    pub const SEQUENCE: usize = super::BLOCK_HEADER_SIZE;
    pub const COMMAND: usize = SEQUENCE + 4;
    pub const ARG0: usize = COMMAND + 4;
    pub const ARG1: usize = ARG0 + 4;
    pub const ARG2: usize = ARG1 + 4;
}

mod status_offset {
    pub use super::header_offset::{CHECKSUM, MAGIC};

    pub const SEQUENCE: usize = super::BLOCK_HEADER_SIZE;
    pub const STEP: usize = SEQUENCE + 4;
    pub const RESULT: usize = STEP + 4;
}

mod plan_offset {
    pub use super::header_offset::{CHECKSUM, MAGIC};

    pub const PAYLOAD_LEN: usize = super::BLOCK_HEADER_SIZE;
    pub const PAYLOAD_CHECKSUM: usize = PAYLOAD_LEN + 4;
    pub const ARGS_LEN: usize = PAYLOAD_CHECKSUM + 4;
    pub const ARGS_CHECKSUM: usize = ARGS_LEN + 4;
    pub const LOADER_LEN: usize = ARGS_CHECKSUM + 4;
    pub const LOADER_CHECKSUM: usize = LOADER_LEN + 4;
}

/// A host-written job block.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashJob {
    pub magic: u32,
    pub version: u32,
    pub size: u32,
    pub checksum: u32,
    pub sequence: u32,
    pub command: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub arg2: u32,
}

/// A flasher-written status block.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashStatus {
    pub magic: u32,
    pub version: u32,
    pub size: u32,
    pub checksum: u32,
    pub sequence: u32,
    pub step: u32,
    pub result: u32,
}

/// A host-written plan for [`FLASH_COMMAND_FLASH_SD`].
///
/// The plan describes the three staged regions: the payload image, the optional
/// `boot.cfg` contents and the sealed SD/SPI loader. It carries lengths and
/// checksums only; the bytes themselves sit at the fixed [`FLASH_PAYLOAD_ADDR`],
/// [`FLASH_ARGS_ADDR`] and [`FLASH_LOADER_ADDR`] addresses.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashPlan {
    pub magic: u32,
    pub version: u32,
    pub size: u32,
    pub checksum: u32,
    pub payload_len: u32,
    pub payload_checksum: u32,
    pub args_len: u32,
    pub args_checksum: u32,
    pub loader_len: u32,
    pub loader_checksum: u32,
}

/// Number of bytes in a [`FlashJob`].
pub const FLASH_JOB_SIZE: usize = 36;
/// Number of bytes in a [`FlashStatus`].
pub const FLASH_STATUS_SIZE: usize = 28;
/// Number of bytes in a [`FlashPlan`].
pub const FLASH_PLAN_SIZE: usize = 40;

/// Why a job or status block was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashContractError {
    /// The input is shorter than the block it claims to contain.
    Truncated,
    /// The block magic does not match its kind.
    BadMagic { got: u32 },
    /// The block version is not [`FLASH_CONTRACT_VERSION`].
    BadVersion { got: u32 },
    /// The declared block size is not the current size.
    BadSize { got: u32 },
    /// The checksum does not cover the rest of the block.
    BadChecksum { expected: u32, got: u32 },
    /// A staged region exceeds its window.
    TooLarge {
        /// Region name for the log.
        field: &'static str,
        /// Declared size.
        size: u32,
        /// Largest accepted size.
        max: u32,
    },
    /// A staged region that must be present is empty.
    Empty {
        /// Region name for the log.
        field: &'static str,
    },
    /// A staged region is not block-aligned.
    BadExtent {
        /// Region name for the log.
        field: &'static str,
        /// Declared size.
        size: u32,
        /// Required alignment.
        align: u32,
    },
}

impl core::fmt::Display for FlashContractError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => f.write_str("flash block is truncated"),
            Self::BadMagic { got } => write!(f, "flash block magic is {got:#010x}"),
            Self::BadVersion { got } => write!(f, "unsupported flash block version {got}"),
            Self::BadSize { got } => write!(f, "flash block size is {got}, not the current size"),
            Self::BadChecksum { expected, got } => write!(
                f,
                "flash block checksum is {got:#010x}, expected {expected:#010x}"
            ),
            Self::TooLarge { field, size, max } => {
                write!(
                    f,
                    "{field} is {size} bytes, larger than the {max}-byte window"
                )
            }
            Self::Empty { field } => write!(f, "{field} must not be empty"),
            Self::BadExtent { field, size, align } => {
                write!(
                    f,
                    "{field} is {size} bytes; it must be a multiple of {align}"
                )
            }
        }
    }
}

fn checksum_without(bytes: &mut [u8], offset: usize) -> u32 {
    let saved = read_u32_le(bytes, offset);
    write_u32_le(bytes, offset, 0);
    let checksum = image_checksum(bytes);
    write_u32_le(bytes, offset, saved);
    checksum
}

impl FlashJob {
    const SPEC: BlockShape = BlockShape::new(FLASH_JOB_MAGIC, FLASH_JOB_SIZE);

    const fn header(&self) -> BlockHeader {
        BlockHeader {
            magic: self.magic,
            version: self.version,
            size: self.size,
            checksum: self.checksum,
        }
    }

    /// Build a job and stamp its magic, version, size and checksum.
    pub fn new(sequence: u32, command: u32, arg0: u32, arg1: u32, arg2: u32) -> Self {
        let header = Self::SPEC.unstamped_header();
        let mut job = Self {
            magic: header.magic,
            version: header.version,
            size: header.size,
            checksum: header.checksum,
            sequence,
            command,
            arg0,
            arg1,
            arg2,
        };
        let mut bytes = job.to_bytes();
        job.checksum = checksum_without(&mut bytes, header_offset::CHECKSUM);
        job
    }

    /// The job's little-endian byte representation, as written to DRAM.
    pub const fn to_bytes(self) -> [u8; FLASH_JOB_SIZE] {
        let mut out = [0u8; FLASH_JOB_SIZE];
        self.header().write(&mut out);
        write_u32_le(&mut out, job_offset::SEQUENCE, self.sequence);
        write_u32_le(&mut out, job_offset::COMMAND, self.command);
        write_u32_le(&mut out, job_offset::ARG0, self.arg0);
        write_u32_le(&mut out, job_offset::ARG1, self.arg1);
        write_u32_le(&mut out, job_offset::ARG2, self.arg2);
        out
    }

    /// Decode and validate a job block.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FlashContractError> {
        if bytes.len() < FLASH_JOB_SIZE {
            return Err(FlashContractError::Truncated);
        }
        let mut raw = [0u8; FLASH_JOB_SIZE];
        raw.copy_from_slice(&bytes[..FLASH_JOB_SIZE]);
        let header = BlockHeader::read(&raw);
        let job = Self {
            magic: header.magic,
            version: header.version,
            size: header.size,
            checksum: header.checksum,
            sequence: read_u32_le(&raw, job_offset::SEQUENCE),
            command: read_u32_le(&raw, job_offset::COMMAND),
            arg0: read_u32_le(&raw, job_offset::ARG0),
            arg1: read_u32_le(&raw, job_offset::ARG1),
            arg2: read_u32_le(&raw, job_offset::ARG2),
        };
        job.verify()?;
        Ok(job)
    }

    /// Check the magic, version, size and checksum.
    pub fn verify(&self) -> Result<(), FlashContractError> {
        self.header().verify(Self::SPEC, &mut self.to_bytes())
    }
}

impl FlashPlan {
    const SPEC: BlockShape = BlockShape::new(FLASH_PLAN_MAGIC, FLASH_PLAN_SIZE);

    const fn header(&self) -> BlockHeader {
        BlockHeader {
            magic: self.magic,
            version: self.version,
            size: self.size,
            checksum: self.checksum,
        }
    }

    /// Build a plan for the three staged regions and stamp its checksums.
    ///
    /// `args` may be empty when the host does not want to write `boot.cfg`;
    /// `payload` and `loader` must be non-empty, and the loader must be a whole
    /// number of SD blocks because it is written directly to sector 16.
    pub fn new(payload: &[u8], args: &[u8], loader: &[u8]) -> Result<Self, FlashContractError> {
        let header = Self::SPEC.unstamped_header();
        let mut plan = Self {
            magic: header.magic,
            version: header.version,
            size: header.size,
            checksum: header.checksum,
            payload_len: payload.len() as u32,
            payload_checksum: image_checksum(payload),
            args_len: args.len() as u32,
            args_checksum: image_checksum(args),
            loader_len: loader.len() as u32,
            loader_checksum: image_checksum(loader),
        };
        plan.verify_extents()?;

        let mut bytes = plan.to_bytes();
        plan.checksum = checksum_without(&mut bytes, header_offset::CHECKSUM);
        Ok(plan)
    }

    /// The plan's little-endian byte representation, as written to DRAM.
    pub const fn to_bytes(self) -> [u8; FLASH_PLAN_SIZE] {
        let mut out = [0u8; FLASH_PLAN_SIZE];
        self.header().write(&mut out);
        write_u32_le(&mut out, plan_offset::PAYLOAD_LEN, self.payload_len);
        write_u32_le(
            &mut out,
            plan_offset::PAYLOAD_CHECKSUM,
            self.payload_checksum,
        );
        write_u32_le(&mut out, plan_offset::ARGS_LEN, self.args_len);
        write_u32_le(&mut out, plan_offset::ARGS_CHECKSUM, self.args_checksum);
        write_u32_le(&mut out, plan_offset::LOADER_LEN, self.loader_len);
        write_u32_le(&mut out, plan_offset::LOADER_CHECKSUM, self.loader_checksum);
        out
    }

    /// Decode and validate a plan block.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FlashContractError> {
        if bytes.len() < FLASH_PLAN_SIZE {
            return Err(FlashContractError::Truncated);
        }
        let mut raw = [0u8; FLASH_PLAN_SIZE];
        raw.copy_from_slice(&bytes[..FLASH_PLAN_SIZE]);
        let header = BlockHeader::read(&raw);
        let plan = Self {
            magic: header.magic,
            version: header.version,
            size: header.size,
            checksum: header.checksum,
            payload_len: read_u32_le(&raw, plan_offset::PAYLOAD_LEN),
            payload_checksum: read_u32_le(&raw, plan_offset::PAYLOAD_CHECKSUM),
            args_len: read_u32_le(&raw, plan_offset::ARGS_LEN),
            args_checksum: read_u32_le(&raw, plan_offset::ARGS_CHECKSUM),
            loader_len: read_u32_le(&raw, plan_offset::LOADER_LEN),
            loader_checksum: read_u32_le(&raw, plan_offset::LOADER_CHECKSUM),
        };
        plan.verify()?;
        Ok(plan)
    }

    /// Check the block envelope and that every declared extent fits its window.
    pub fn verify(&self) -> Result<(), FlashContractError> {
        self.header().verify(Self::SPEC, &mut self.to_bytes())?;
        self.verify_extents()
    }

    /// Whether the three slices are exactly the staged regions this plan names.
    pub fn matches(&self, payload: &[u8], args: &[u8], loader: &[u8]) -> bool {
        self.payload_len as usize == payload.len()
            && self.args_len as usize == args.len()
            && self.loader_len as usize == loader.len()
            && self.payload_checksum == image_checksum(payload)
            && self.args_checksum == image_checksum(args)
            && self.loader_checksum == image_checksum(loader)
    }

    /// Whether the plan asks the flasher to write `boot.cfg`.
    pub const fn has_args(&self) -> bool {
        self.args_len != 0
    }

    fn verify_extents(&self) -> Result<(), FlashContractError> {
        if self.payload_len == 0 {
            return Err(FlashContractError::Empty {
                field: "payload image",
            });
        }
        if self.payload_len > FLASH_PAYLOAD_MAX_SIZE {
            return Err(FlashContractError::TooLarge {
                field: "payload image",
                size: self.payload_len,
                max: FLASH_PAYLOAD_MAX_SIZE,
            });
        }
        if self.args_len > FLASH_ARGS_MAX_SIZE {
            return Err(FlashContractError::TooLarge {
                field: "boot.cfg contents",
                size: self.args_len,
                max: FLASH_ARGS_MAX_SIZE,
            });
        }
        if self.loader_len == 0 {
            return Err(FlashContractError::Empty { field: "SD loader" });
        }
        if self.loader_len > FLASH_LOADER_MAX_SIZE {
            return Err(FlashContractError::TooLarge {
                field: "SD loader",
                size: self.loader_len,
                max: FLASH_LOADER_MAX_SIZE,
            });
        }
        if !self.loader_len.is_multiple_of(FLASH_SECTOR_SIZE as u32) {
            return Err(FlashContractError::BadExtent {
                field: "SD loader",
                size: self.loader_len,
                align: FLASH_SECTOR_SIZE as u32,
            });
        }
        Ok(())
    }
}

impl FlashStatus {
    const SPEC: BlockShape = BlockShape::new(FLASH_STATUS_MAGIC, FLASH_STATUS_SIZE);

    const fn header(&self) -> BlockHeader {
        BlockHeader {
            magic: self.magic,
            version: self.version,
            size: self.size,
            checksum: self.checksum,
        }
    }

    /// Build a status and stamp its magic, version, size and checksum.
    pub fn new(sequence: u32, step: u32, result: u32) -> Self {
        let header = Self::SPEC.unstamped_header();
        let mut status = Self {
            magic: header.magic,
            version: header.version,
            size: header.size,
            checksum: header.checksum,
            sequence,
            step,
            result,
        };
        let mut bytes = status.to_bytes();
        status.checksum = checksum_without(&mut bytes, header_offset::CHECKSUM);
        status
    }

    /// The status's little-endian byte representation, as written to DRAM.
    pub const fn to_bytes(self) -> [u8; FLASH_STATUS_SIZE] {
        let mut out = [0u8; FLASH_STATUS_SIZE];
        self.header().write(&mut out);
        write_u32_le(&mut out, status_offset::SEQUENCE, self.sequence);
        write_u32_le(&mut out, status_offset::STEP, self.step);
        write_u32_le(&mut out, status_offset::RESULT, self.result);
        out
    }

    /// Decode and validate a status block.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FlashContractError> {
        if bytes.len() < FLASH_STATUS_SIZE {
            return Err(FlashContractError::Truncated);
        }
        let mut raw = [0u8; FLASH_STATUS_SIZE];
        raw.copy_from_slice(&bytes[..FLASH_STATUS_SIZE]);
        let header = BlockHeader::read(&raw);
        let status = Self {
            magic: header.magic,
            version: header.version,
            size: header.size,
            checksum: header.checksum,
            sequence: read_u32_le(&raw, status_offset::SEQUENCE),
            step: read_u32_le(&raw, status_offset::STEP),
            result: read_u32_le(&raw, status_offset::RESULT),
        };
        status.verify()?;
        Ok(status)
    }

    /// Check the magic, version, size and checksum.
    pub fn verify(&self) -> Result<(), FlashContractError> {
        self.header().verify(Self::SPEC, &mut self.to_bytes())
    }

    /// Whether the flasher reported a completed, successful job.
    pub const fn is_ok(&self) -> bool {
        self.step == FLASH_STEP_DONE && self.result == FLASH_RESULT_OK
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockHeader {
    magic: u32,
    version: u32,
    size: u32,
    checksum: u32,
}

impl BlockHeader {
    const fn new(magic: u32, size: usize) -> Self {
        Self {
            magic,
            version: FLASH_CONTRACT_VERSION,
            size: size as u32,
            checksum: 0,
        }
    }

    const fn write(self, out: &mut [u8]) {
        write_u32_le(out, header_offset::MAGIC, self.magic);
        write_u32_le(out, header_offset::VERSION, self.version);
        write_u32_le(out, header_offset::SIZE, self.size);
        write_u32_le(out, header_offset::CHECKSUM, self.checksum);
    }

    fn read(bytes: &[u8]) -> Self {
        Self {
            magic: read_u32_le(bytes, header_offset::MAGIC),
            version: read_u32_le(bytes, header_offset::VERSION),
            size: read_u32_le(bytes, header_offset::SIZE),
            checksum: read_u32_le(bytes, header_offset::CHECKSUM),
        }
    }

    fn verify(self, spec: BlockShape, bytes: &mut [u8]) -> Result<(), FlashContractError> {
        if self.magic != spec.magic {
            return Err(FlashContractError::BadMagic { got: self.magic });
        }
        if self.version != FLASH_CONTRACT_VERSION {
            return Err(FlashContractError::BadVersion { got: self.version });
        }
        if self.size != spec.size as u32 {
            return Err(FlashContractError::BadSize { got: self.size });
        }
        let expected = checksum_without(bytes, header_offset::CHECKSUM);
        if self.checksum != expected {
            return Err(FlashContractError::BadChecksum {
                expected,
                got: self.checksum,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct BlockShape {
    magic: u32,
    size: usize,
}

impl BlockShape {
    const fn new(magic: u32, size: usize) -> Self {
        Self { magic, size }
    }

    const fn unstamped_header(self) -> BlockHeader {
        BlockHeader::new(self.magic, self.size)
    }
}

/// Step values and their log names; the order is not significant.
const STEP_NAMES: &[(u32, &str)] = &[
    (FLASH_STEP_DONE, "done"),
    (FLASH_STEP_PLAN, "plan"),
    (FLASH_STEP_SD_INIT, "SD init"),
    (FLASH_STEP_LAYOUT, "layout"),
    (FLASH_STEP_WRITE_PAYLOAD, "write payload"),
    (FLASH_STEP_WRITE_ARGS, "write boot.cfg"),
    (FLASH_STEP_VERIFY_FAT, "verify FAT files"),
    (FLASH_STEP_WRITE_LOADER, "write loader"),
    (FLASH_STEP_VERIFY_LOADER, "verify loader"),
    (FLASH_STEP_READ_MBR, "read MBR"),
    (FLASH_STEP_RAW_WRITE, "raw write"),
    (FLASH_STEP_RAW_VERIFY, "raw verify"),
];

/// Result values and their log names; the order is not significant.
const RESULT_NAMES: &[(u32, &str)] = &[
    (FLASH_RESULT_OK, "ok"),
    (FLASH_RESULT_BAD_JOB, "bad job"),
    (FLASH_RESULT_UNKNOWN_COMMAND, "unknown command"),
    (FLASH_RESULT_BAD_PLAN, "bad plan or staged image"),
    (FLASH_RESULT_SD_OFFLINE, "SD card offline"),
    (FLASH_RESULT_NO_MBR, "no MBR"),
    (FLASH_RESULT_NO_FAT, "no FAT partition"),
    (FLASH_RESULT_UNSAFE_LAYOUT, "unsafe card layout"),
    (FLASH_RESULT_CARD_TOO_SMALL, "card too small"),
    (FLASH_RESULT_WRITE_FAILED, "write failed"),
    (FLASH_RESULT_VERIFY_FAILED, "read-back mismatch"),
    (FLASH_RESULT_MBR_CHANGED, "MBR changed"),
    (FLASH_RESULT_READ_FAILED, "read failed"),
    (FLASH_RESULT_FILE_CREATE, "create file failed"),
    (FLASH_RESULT_FILE_TRUNCATE, "truncate failed"),
    (FLASH_RESULT_FILE_WRITE, "file write failed"),
    (FLASH_RESULT_FILE_FLUSH, "file flush failed"),
];

/// A human-readable name for a status step, for logs.
pub const fn step_name(step: u32) -> &'static str {
    let mut index = 0;
    while index < STEP_NAMES.len() {
        if STEP_NAMES[index].0 == step {
            return STEP_NAMES[index].1;
        }
        index += 1;
    }
    "unknown"
}

/// A human-readable name for a status result, for logs.
pub const fn result_name(result: u32) -> &'static str {
    let mut index = 0;
    while index < RESULT_NAMES.len() {
        if RESULT_NAMES[index].0 == result {
            return RESULT_NAMES[index].1;
        }
        index += 1;
    }
    "unknown"
}

const _: () = {
    use core::mem::{offset_of, size_of};

    assert!(BLOCK_HEADER_SIZE == header_offset::CHECKSUM + 4);
    assert!(size_of::<DdrResult>() == DDR_RESULT_SIZE);
    assert!(size_of::<FlashJob>() == FLASH_JOB_SIZE);
    assert!(size_of::<FlashStatus>() == FLASH_STATUS_SIZE);
    assert!(size_of::<FlashPlan>() == FLASH_PLAN_SIZE);
    assert!(offset_of!(DdrResult, magic) == 0);
    assert!(offset_of!(DdrResult, size_mib) == 4);
    assert!(offset_of!(FlashJob, magic) == job_offset::MAGIC);
    assert!(offset_of!(FlashJob, checksum) == job_offset::CHECKSUM);
    assert!(offset_of!(FlashJob, sequence) == job_offset::SEQUENCE);
    assert!(offset_of!(FlashJob, arg2) == job_offset::ARG2);
    assert!(offset_of!(FlashStatus, magic) == status_offset::MAGIC);
    assert!(offset_of!(FlashStatus, checksum) == status_offset::CHECKSUM);
    assert!(offset_of!(FlashStatus, result) == status_offset::RESULT);
    assert!(offset_of!(FlashPlan, magic) == plan_offset::MAGIC);
    assert!(offset_of!(FlashPlan, checksum) == plan_offset::CHECKSUM);
    assert!(offset_of!(FlashPlan, loader_checksum) == plan_offset::LOADER_CHECKSUM);

    assert!(DDR_RESULT_ADDR == DDR_STUB_LOAD_ADDR + DDR_STUB_MAX_SIZE);
    assert!(DDR_RESULT_ADDR + 8 <= FEL_MAILBOX_BASE);
    assert!(FLASH_JOB_ADDR + FLASH_JOB_SIZE as u32 <= FLASH_STATUS_ADDR);
    assert!(FLASH_STATUS_ADDR + FLASH_STATUS_SIZE as u32 <= FLASH_PLAN_ADDR);
    assert!(FLASH_PLAN_ADDR + FLASH_PLAN_SIZE as u32 <= DRAM_TEST_ADDR);
    assert!(DRAM_TEST_ADDR + DRAM_TEST_SIZE <= FLASHER_LOAD_ADDR);
    assert!(FLASHER_LOAD_ADDR + FLASHER_WINDOW_SIZE == FLASHER_STACK_TOP);
    assert!(FLASHER_MAX_SIZE + FLASHER_STACK_SIZE == FLASHER_WINDOW_SIZE);
    assert!(FLASHER_STACK_TOP <= FLASH_PAYLOAD_ADDR);
    assert!(FLASH_PAYLOAD_ADDR + FLASH_PAYLOAD_MAX_SIZE == FLASH_ARGS_ADDR);
    assert!(FLASH_ARGS_ADDR + FLASH_ARGS_MAX_SIZE <= FLASH_LOADER_ADDR);
    assert!(FLASH_LOADER_ADDR + FLASH_LOADER_MAX_SIZE == FLASH_STAGE_END);
    assert!(FLASH_STAGE_END - FLASH_DRAM_BASE <= 128 * 1024 * 1024);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ddr_result_round_trips() {
        let result = DdrResult::new(128);
        assert_eq!(DdrResult::from_bytes(&result.to_bytes()), Some(result));
    }

    #[test]
    fn a_ddr_result_without_its_magic_is_not_a_result() {
        assert_eq!(DdrResult::from_bytes(&[0u8; DDR_RESULT_SIZE]), None);
        assert_eq!(DdrResult::from_bytes(&[0u8; DDR_RESULT_SIZE - 1]), None);
    }

    #[test]
    fn a_new_job_round_trips() {
        let job = FlashJob::new(7, FLASH_COMMAND_NOOP, 0x11, 0x22, 0x33);
        let bytes = job.to_bytes();

        assert_eq!(job.magic, FLASH_JOB_MAGIC);
        assert_eq!(job.version, FLASH_CONTRACT_VERSION);
        assert_eq!(job.size, FLASH_JOB_SIZE as u32);
        assert_eq!(FlashJob::from_bytes(&bytes), Ok(job));
    }

    #[test]
    fn a_new_status_round_trips() {
        let status = FlashStatus::new(7, FLASH_STEP_DONE, FLASH_RESULT_OK);
        let bytes = status.to_bytes();

        assert_eq!(status.magic, FLASH_STATUS_MAGIC);
        assert_eq!(status.size, FLASH_STATUS_SIZE as u32);
        assert_eq!(FlashStatus::from_bytes(&bytes), Ok(status));
        assert!(status.is_ok());
    }

    #[test]
    fn an_incomplete_status_is_not_success() {
        let status = FlashStatus::new(7, FLASH_STEP_PLAN, FLASH_RESULT_OK);
        assert!(!status.is_ok());
    }

    #[test]
    fn every_job_field_changes_the_checksum() {
        let base = FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 0);
        for job in [
            FlashJob::new(2, FLASH_COMMAND_NOOP, 0, 0, 0),
            FlashJob::new(1, 1, 0, 0, 0),
            FlashJob::new(1, FLASH_COMMAND_NOOP, 1, 0, 0),
            FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 1, 0),
            FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 1),
        ] {
            assert_ne!(job.checksum, base.checksum, "{job:?}");
        }
    }

    #[test]
    fn every_status_field_changes_the_checksum() {
        let base = FlashStatus::new(1, FLASH_STEP_DONE, FLASH_RESULT_OK);
        for status in [
            FlashStatus::new(2, FLASH_STEP_DONE, FLASH_RESULT_OK),
            FlashStatus::new(1, FLASH_STEP_PLAN, FLASH_RESULT_OK),
            FlashStatus::new(1, FLASH_STEP_DONE, FLASH_RESULT_BAD_JOB),
        ] {
            assert_ne!(status.checksum, base.checksum, "{status:?}");
        }
    }

    fn loader(len: usize) -> Vec<u8> {
        (0..len).map(|index| index as u8).collect()
    }

    fn sample_job() -> FlashJob {
        FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 0)
    }

    fn sample_status() -> FlashStatus {
        FlashStatus::new(1, FLASH_STEP_DONE, FLASH_RESULT_OK)
    }

    fn sample_plan() -> FlashPlan {
        FlashPlan::new(b"payload", b"args", &loader(512)).unwrap()
    }

    #[test]
    fn rejects_a_truncated_block() {
        assert_eq!(
            FlashJob::from_bytes(&[0u8; FLASH_JOB_SIZE - 1]),
            Err(FlashContractError::Truncated)
        );
        assert_eq!(
            FlashStatus::from_bytes(&[0u8; FLASH_STATUS_SIZE - 1]),
            Err(FlashContractError::Truncated)
        );
        assert_eq!(
            FlashPlan::from_bytes(&[0u8; FLASH_PLAN_SIZE - 1]),
            Err(FlashContractError::Truncated)
        );
    }

    #[test]
    fn rejects_a_bad_magic_in_every_block() {
        let mut job = sample_job().to_bytes();
        job[header_offset::MAGIC] ^= 0xff;
        assert!(matches!(
            FlashJob::from_bytes(&job),
            Err(FlashContractError::BadMagic { .. })
        ));

        let mut status = sample_status().to_bytes();
        status[header_offset::MAGIC] ^= 0xff;
        assert!(matches!(
            FlashStatus::from_bytes(&status),
            Err(FlashContractError::BadMagic { .. })
        ));

        let mut plan = sample_plan().to_bytes();
        plan[header_offset::MAGIC] ^= 0xff;
        assert!(matches!(
            FlashPlan::from_bytes(&plan),
            Err(FlashContractError::BadMagic { .. })
        ));
    }

    #[test]
    fn rejects_a_bad_version_in_every_block() {
        let mut job = sample_job().to_bytes();
        write_u32_le(&mut job, header_offset::VERSION, 2);
        assert_eq!(
            FlashJob::from_bytes(&job),
            Err(FlashContractError::BadVersion { got: 2 })
        );

        let mut status = sample_status().to_bytes();
        write_u32_le(&mut status, header_offset::VERSION, 2);
        assert_eq!(
            FlashStatus::from_bytes(&status),
            Err(FlashContractError::BadVersion { got: 2 })
        );

        let mut plan = sample_plan().to_bytes();
        write_u32_le(&mut plan, header_offset::VERSION, 2);
        assert_eq!(
            FlashPlan::from_bytes(&plan),
            Err(FlashContractError::BadVersion { got: 2 })
        );
    }

    #[test]
    fn rejects_a_bad_size_in_every_block() {
        let mut job = sample_job().to_bytes();
        write_u32_le(&mut job, header_offset::SIZE, 4);
        assert_eq!(
            FlashJob::from_bytes(&job),
            Err(FlashContractError::BadSize { got: 4 })
        );

        let mut status = sample_status().to_bytes();
        write_u32_le(&mut status, header_offset::SIZE, 4);
        assert_eq!(
            FlashStatus::from_bytes(&status),
            Err(FlashContractError::BadSize { got: 4 })
        );

        let mut plan = sample_plan().to_bytes();
        write_u32_le(&mut plan, header_offset::SIZE, 4);
        assert_eq!(
            FlashPlan::from_bytes(&plan),
            Err(FlashContractError::BadSize { got: 4 })
        );
    }

    #[test]
    fn rejects_a_bad_checksum_in_every_block() {
        let mut job = sample_job().to_bytes();
        job[job_offset::ARG0] ^= 0xff;
        assert!(matches!(
            FlashJob::from_bytes(&job),
            Err(FlashContractError::BadChecksum { .. })
        ));

        let mut status = sample_status().to_bytes();
        status[status_offset::RESULT] ^= 0xff;
        assert!(matches!(
            FlashStatus::from_bytes(&status),
            Err(FlashContractError::BadChecksum { .. })
        ));

        let mut plan = sample_plan().to_bytes();
        plan[plan_offset::LOADER_LEN] ^= 0xff;
        assert!(matches!(
            FlashPlan::from_bytes(&plan),
            Err(FlashContractError::BadChecksum { .. })
        ));
    }

    #[test]
    fn every_plan_field_changes_the_checksum() {
        let base = sample_plan();
        let mut other_loader = loader(512);
        other_loader[0] ^= 0xff;
        let mut other_payload = b"payload".to_vec();
        other_payload[0] ^= 0xff;
        let mut other_args = b"args".to_vec();
        other_args[0] ^= 0xff;

        for plan in [
            FlashPlan::new(b"payload!", b"args", &loader(512)).unwrap(),
            FlashPlan::new(&other_payload, b"args", &loader(512)).unwrap(),
            FlashPlan::new(b"payload", b"args!", &loader(512)).unwrap(),
            FlashPlan::new(b"payload", &other_args, &loader(512)).unwrap(),
            FlashPlan::new(b"payload", b"args", &loader(1024)).unwrap(),
            FlashPlan::new(b"payload", b"args", &other_loader).unwrap(),
        ] {
            assert_ne!(plan.checksum, base.checksum, "{plan:?}");
        }
    }

    #[test]
    fn a_flash_plan_round_trips_and_matches_its_regions() {
        let payload = b"wrapped payload image";
        let args = b"console=ttyS0";
        let loader = loader(1024);

        let plan = FlashPlan::new(payload, args, loader.as_slice()).unwrap();
        let bytes = plan.to_bytes();

        assert_eq!(plan.magic, FLASH_PLAN_MAGIC);
        assert_eq!(plan.size, FLASH_PLAN_SIZE as u32);
        assert_eq!(FlashPlan::from_bytes(&bytes), Ok(plan));
        assert!(plan.has_args());
        assert!(plan.matches(payload, args, &loader));
    }

    #[test]
    fn an_empty_boot_cfg_is_allowed_and_plan_has_no_args() {
        let plan = FlashPlan::new(b"payload", &[], &loader(512)).unwrap();

        assert!(!plan.has_args());
        assert!(plan.matches(b"payload", &[], &loader(512)));
    }

    #[test]
    fn a_plan_rejects_a_mismatched_region() {
        let payload = b"payload";
        let loader = loader(512);
        let plan = FlashPlan::new(payload, b"args", &loader).unwrap();

        assert!(!plan.matches(b"other", b"args", &loader));
        assert!(!plan.matches(payload, b"other", &loader));
        assert!(!plan.matches(payload, b"args", &loader[..511]));
    }

    #[test]
    fn a_plan_rejects_an_empty_payload_or_loader() {
        assert_eq!(
            FlashPlan::new(&[], &[], &loader(512)),
            Err(FlashContractError::Empty {
                field: "payload image",
            })
        );
        assert_eq!(
            FlashPlan::new(b"payload", &[], &[]),
            Err(FlashContractError::Empty { field: "SD loader" })
        );
    }

    #[test]
    fn a_plan_rejects_a_loader_that_is_not_block_aligned() {
        assert_eq!(
            FlashPlan::new(b"payload", &[], &loader(513)),
            Err(FlashContractError::BadExtent {
                field: "SD loader",
                size: 513,
                align: FLASH_SECTOR_SIZE as u32,
            })
        );
    }

    #[test]
    fn a_plan_rejects_an_oversized_region() {
        assert!(matches!(
            FlashPlan::new(
                &vec![0u8; FLASH_PAYLOAD_MAX_SIZE as usize + 1],
                &[],
                &loader(512)
            ),
            Err(FlashContractError::TooLarge {
                field: "payload image",
                ..
            })
        ));
        assert!(matches!(
            FlashPlan::new(
                b"payload",
                &vec![0u8; FLASH_ARGS_MAX_SIZE as usize + 1],
                &loader(512)
            ),
            Err(FlashContractError::TooLarge {
                field: "boot.cfg contents",
                ..
            })
        ));
        assert!(matches!(
            FlashPlan::new(
                b"payload",
                &[],
                &vec![0u8; FLASH_LOADER_MAX_SIZE as usize + 512]
            ),
            Err(FlashContractError::TooLarge {
                field: "SD loader",
                ..
            })
        ));
    }

    #[test]
    fn accepts_a_loader_that_ends_at_the_partition_start() {
        // Sector 17 starts at 8704 bytes; a 512-byte loader at sector 16 ends
        // exactly there.
        assert_eq!(check_sd_layout(17, 4096, 512, 8704 + 4096), Ok(()));
    }

    #[test]
    fn rejects_a_partition_at_or_inside_sector_16() {
        assert_eq!(
            check_sd_layout(16, 4096, 512, 100_000),
            Err(FlashLayoutError::PartitionTooEarly)
        );
        assert_eq!(
            check_sd_layout(15, 4096, 512, 100_000),
            Err(FlashLayoutError::PartitionTooEarly)
        );
    }

    #[test]
    fn rejects_a_loader_that_reaches_into_the_partition() {
        assert_eq!(
            check_sd_layout(17, 4096, 1024, 100_000),
            Err(FlashLayoutError::LoaderOverlapsPartition)
        );
    }

    #[test]
    fn rejects_a_partition_that_extends_past_the_card() {
        assert_eq!(
            check_sd_layout(17, 4096, 512, 8704 + 4095),
            Err(FlashLayoutError::PartitionBeyondCard)
        );
    }

    #[test]
    fn names_every_step_and_result() {
        for &(step, name) in STEP_NAMES {
            assert_eq!(step_name(step), name);
        }
        for &(result, name) in RESULT_NAMES {
            assert_eq!(result_name(result), name);
        }
    }
}
