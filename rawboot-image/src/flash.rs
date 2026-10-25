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

/// The block layout version shared by jobs and statuses.
pub const FLASH_CONTRACT_VERSION: u32 = 1;

/// `RBFJ` in little-endian order: a host-written flash job.
pub const FLASH_JOB_MAGIC: u32 = u32::from_le_bytes(*b"RBFJ");
/// `RBFS` in little-endian order: a flasher-written status.
pub const FLASH_STATUS_MAGIC: u32 = u32::from_le_bytes(*b"RBFS");

/// There is no work to do beyond proving the path; used by the #41 spike.
pub const FLASH_COMMAND_NOOP: u32 = 0;
/// The flasher finished successfully.
pub const FLASH_RESULT_OK: u32 = 0;
/// The flasher could not parse the job block.
pub const FLASH_RESULT_BAD_JOB: u32 = 1;
/// The job names a command this flasher does not implement.
pub const FLASH_RESULT_UNKNOWN_COMMAND: u32 = 2;
/// The flasher has read the job and is executing it.
pub const FLASH_STEP_STARTED: u32 = 1;
/// The flasher has written its final status.
pub const FLASH_STEP_DONE: u32 = 2;

mod job_offset {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 4;
    pub const SIZE: usize = 8;
    pub const CHECKSUM: usize = 12;
    pub const SEQUENCE: usize = 16;
    pub const COMMAND: usize = 20;
    pub const ARG0: usize = 24;
    pub const ARG1: usize = 28;
    pub const ARG2: usize = 32;
}

mod status_offset {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 4;
    pub const SIZE: usize = 8;
    pub const CHECKSUM: usize = 12;
    pub const SEQUENCE: usize = 16;
    pub const STEP: usize = 20;
    pub const RESULT: usize = 24;
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

/// Number of bytes in a [`FlashJob`].
pub const FLASH_JOB_SIZE: usize = 36;
/// Number of bytes in a [`FlashStatus`].
pub const FLASH_STATUS_SIZE: usize = 28;

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
    /// Build a job and stamp its magic, version, size and checksum.
    pub fn new(sequence: u32, command: u32, arg0: u32, arg1: u32, arg2: u32) -> Self {
        let mut job = Self {
            magic: FLASH_JOB_MAGIC,
            version: FLASH_CONTRACT_VERSION,
            size: FLASH_JOB_SIZE as u32,
            checksum: 0,
            sequence,
            command,
            arg0,
            arg1,
            arg2,
        };
        let mut bytes = job.to_bytes();
        job.checksum = checksum_without(&mut bytes, job_offset::CHECKSUM);
        job
    }

    /// The job's little-endian byte representation, as written to DRAM.
    pub const fn to_bytes(self) -> [u8; FLASH_JOB_SIZE] {
        let mut out = [0u8; FLASH_JOB_SIZE];
        write_u32_le(&mut out, job_offset::MAGIC, self.magic);
        write_u32_le(&mut out, job_offset::VERSION, self.version);
        write_u32_le(&mut out, job_offset::SIZE, self.size);
        write_u32_le(&mut out, job_offset::CHECKSUM, self.checksum);
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
        let job = Self {
            magic: read_u32_le(&raw, job_offset::MAGIC),
            version: read_u32_le(&raw, job_offset::VERSION),
            size: read_u32_le(&raw, job_offset::SIZE),
            checksum: read_u32_le(&raw, job_offset::CHECKSUM),
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
        verify_block(
            BlockSpec {
                magic: FLASH_JOB_MAGIC,
                size: FLASH_JOB_SIZE,
                checksum_offset: job_offset::CHECKSUM,
            },
            self.magic,
            self.version,
            self.size,
            self.checksum,
            &mut self.to_bytes(),
        )
    }
}

impl FlashStatus {
    /// Build a status and stamp its magic, version, size and checksum.
    pub fn new(sequence: u32, step: u32, result: u32) -> Self {
        let mut status = Self {
            magic: FLASH_STATUS_MAGIC,
            version: FLASH_CONTRACT_VERSION,
            size: FLASH_STATUS_SIZE as u32,
            checksum: 0,
            sequence,
            step,
            result,
        };
        let mut bytes = status.to_bytes();
        status.checksum = checksum_without(&mut bytes, status_offset::CHECKSUM);
        status
    }

    /// The status's little-endian byte representation, as written to DRAM.
    pub const fn to_bytes(self) -> [u8; FLASH_STATUS_SIZE] {
        let mut out = [0u8; FLASH_STATUS_SIZE];
        write_u32_le(&mut out, status_offset::MAGIC, self.magic);
        write_u32_le(&mut out, status_offset::VERSION, self.version);
        write_u32_le(&mut out, status_offset::SIZE, self.size);
        write_u32_le(&mut out, status_offset::CHECKSUM, self.checksum);
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
        let status = Self {
            magic: read_u32_le(&raw, status_offset::MAGIC),
            version: read_u32_le(&raw, status_offset::VERSION),
            size: read_u32_le(&raw, status_offset::SIZE),
            checksum: read_u32_le(&raw, status_offset::CHECKSUM),
            sequence: read_u32_le(&raw, status_offset::SEQUENCE),
            step: read_u32_le(&raw, status_offset::STEP),
            result: read_u32_le(&raw, status_offset::RESULT),
        };
        status.verify()?;
        Ok(status)
    }

    /// Check the magic, version, size and checksum.
    pub fn verify(&self) -> Result<(), FlashContractError> {
        verify_block(
            BlockSpec {
                magic: FLASH_STATUS_MAGIC,
                size: FLASH_STATUS_SIZE,
                checksum_offset: status_offset::CHECKSUM,
            },
            self.magic,
            self.version,
            self.size,
            self.checksum,
            &mut self.to_bytes(),
        )
    }

    /// Whether the flasher reported a completed, successful job.
    pub const fn is_ok(&self) -> bool {
        self.step == FLASH_STEP_DONE && self.result == FLASH_RESULT_OK
    }
}

struct BlockSpec {
    magic: u32,
    size: usize,
    checksum_offset: usize,
}

fn verify_block(
    spec: BlockSpec,
    magic: u32,
    version: u32,
    size: u32,
    checksum: u32,
    bytes: &mut [u8],
) -> Result<(), FlashContractError> {
    if magic != spec.magic {
        return Err(FlashContractError::BadMagic { got: magic });
    }
    if version != FLASH_CONTRACT_VERSION {
        return Err(FlashContractError::BadVersion { got: version });
    }
    if size != spec.size as u32 {
        return Err(FlashContractError::BadSize { got: size });
    }
    let expected = checksum_without(bytes, spec.checksum_offset);
    if checksum != expected {
        return Err(FlashContractError::BadChecksum {
            expected,
            got: checksum,
        });
    }
    Ok(())
}

/// A human-readable name for a status step, for logs.
pub const fn step_name(step: u32) -> &'static str {
    match step {
        FLASH_STEP_STARTED => "started",
        FLASH_STEP_DONE => "done",
        _ => "unknown",
    }
}

/// A human-readable name for a status result, for logs.
pub const fn result_name(result: u32) -> &'static str {
    match result {
        FLASH_RESULT_OK => "ok",
        FLASH_RESULT_BAD_JOB => "bad job",
        FLASH_RESULT_UNKNOWN_COMMAND => "unknown command",
        _ => "unknown",
    }
}

const _: () = {
    use core::mem::{offset_of, size_of};

    assert!(size_of::<DdrResult>() == DDR_RESULT_SIZE);
    assert!(size_of::<FlashJob>() == FLASH_JOB_SIZE);
    assert!(size_of::<FlashStatus>() == FLASH_STATUS_SIZE);
    assert!(offset_of!(DdrResult, magic) == 0);
    assert!(offset_of!(DdrResult, size_mib) == 4);
    assert!(offset_of!(FlashJob, magic) == job_offset::MAGIC);
    assert!(offset_of!(FlashJob, checksum) == job_offset::CHECKSUM);
    assert!(offset_of!(FlashJob, sequence) == job_offset::SEQUENCE);
    assert!(offset_of!(FlashJob, arg2) == job_offset::ARG2);
    assert!(offset_of!(FlashStatus, magic) == status_offset::MAGIC);
    assert!(offset_of!(FlashStatus, checksum) == status_offset::CHECKSUM);
    assert!(offset_of!(FlashStatus, result) == status_offset::RESULT);

    assert!(DDR_RESULT_ADDR == DDR_STUB_LOAD_ADDR + DDR_STUB_MAX_SIZE);
    assert!(DDR_RESULT_ADDR + 8 <= FEL_MAILBOX_BASE);
    assert!(FLASH_JOB_ADDR + FLASH_JOB_SIZE as u32 <= FLASH_STATUS_ADDR);
    assert!(FLASH_STATUS_ADDR + FLASH_STATUS_SIZE as u32 <= DRAM_TEST_ADDR);
    assert!(DRAM_TEST_ADDR + DRAM_TEST_SIZE <= FLASHER_LOAD_ADDR);
    assert!(FLASHER_LOAD_ADDR + FLASHER_WINDOW_SIZE == FLASHER_STACK_TOP);
    assert!(FLASHER_MAX_SIZE + FLASHER_STACK_SIZE == FLASHER_WINDOW_SIZE);
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
        let status = FlashStatus::new(7, FLASH_STEP_STARTED, FLASH_RESULT_OK);
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
    fn rejects_a_truncated_block() {
        assert_eq!(
            FlashJob::from_bytes(&[0u8; FLASH_JOB_SIZE - 1]),
            Err(FlashContractError::Truncated)
        );
        assert_eq!(
            FlashStatus::from_bytes(&[0u8; FLASH_STATUS_SIZE - 1]),
            Err(FlashContractError::Truncated)
        );
    }

    #[test]
    fn rejects_a_bad_magic() {
        let mut bytes = FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 0).to_bytes();
        bytes[0] ^= 0xff;
        assert!(matches!(
            FlashJob::from_bytes(&bytes),
            Err(FlashContractError::BadMagic { .. })
        ));
    }

    #[test]
    fn rejects_a_bad_version() {
        let mut bytes = FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 0).to_bytes();
        write_u32_le(&mut bytes, job_offset::VERSION, 2);
        assert_eq!(
            FlashJob::from_bytes(&bytes),
            Err(FlashContractError::BadVersion { got: 2 })
        );
    }

    #[test]
    fn rejects_a_bad_size() {
        let mut bytes = FlashStatus::new(1, FLASH_STEP_DONE, FLASH_RESULT_OK).to_bytes();
        write_u32_le(&mut bytes, status_offset::SIZE, 4);
        assert_eq!(
            FlashStatus::from_bytes(&bytes),
            Err(FlashContractError::BadSize { got: 4 })
        );
    }

    #[test]
    fn rejects_a_corrupted_payload_field() {
        let mut bytes = FlashStatus::new(1, FLASH_STEP_DONE, FLASH_RESULT_OK).to_bytes();
        bytes[status_offset::RESULT] ^= 0xff;
        assert!(matches!(
            FlashStatus::from_bytes(&bytes),
            Err(FlashContractError::BadChecksum { .. })
        ));
    }
}
