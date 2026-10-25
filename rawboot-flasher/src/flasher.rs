// SPDX-License-Identifier: Unlicense

//! The minimal flasher entry: read the DRAM job, write the DRAM status, return.
//!
//! This is the #41 skeleton, not the SD writer yet. It proves the contract and
//! the return path: the host can read a checksummed status after `exec` hands
//! control back to the BROM. SD and FAT work arrives with #42.

use core::arch::asm;

use rawboot_image::{
    FLASH_COMMAND_NOOP, FLASH_JOB_ADDR, FLASH_JOB_SIZE, FLASH_RESULT_BAD_JOB, FLASH_RESULT_OK,
    FLASH_RESULT_UNKNOWN_COMMAND, FLASH_STATUS_ADDR, FLASH_STEP_DONE, FlashContractError, FlashJob,
    FlashStatus,
};
use sunxi_pl::io::{read8, write8};

const SEQUENCE_OFFSET: u32 = core::mem::offset_of!(FlashJob, sequence) as u32;

/// Run the job currently in DRAM, then publish its status and return.
pub fn run() {
    let job = match read_job() {
        Ok(job) => job,
        Err(_) => {
            // The header is untrustworthy, but the host still needs the status:
            // echo the raw sequence word so a valid host job can match it.
            write_status(raw_sequence(), FLASH_RESULT_BAD_JOB);
            return;
        }
    };

    let result = match job.command {
        FLASH_COMMAND_NOOP => FLASH_RESULT_OK,
        _ => FLASH_RESULT_UNKNOWN_COMMAND,
    };
    write_status(job.sequence, result);
}

fn read_job() -> Result<FlashJob, FlashContractError> {
    let mut bytes = [0u8; FLASH_JOB_SIZE];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = read8(FLASH_JOB_ADDR + index as u32);
    }
    FlashJob::from_bytes(&bytes)
}

fn raw_sequence() -> u32 {
    let mut bytes = [0u8; 4];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = read8(FLASH_JOB_ADDR + SEQUENCE_OFFSET + index as u32);
    }
    u32::from_le_bytes(bytes)
}

fn write_status(sequence: u32, result: u32) {
    let status = FlashStatus::new(sequence, FLASH_STEP_DONE, result);
    for (index, byte) in status.to_bytes().iter().enumerate() {
        write8(FLASH_STATUS_ADDR + index as u32, *byte);
    }
    // The host reads through the BROM after this function returns; order the
    // status stores before the return.
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}
