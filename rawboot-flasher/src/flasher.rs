// SPDX-License-Identifier: Unlicense

//! The flasher entry: read the DRAM job, run it, write the DRAM status, return.
//!
//! [`FLASH_COMMAND_NOOP`] keeps the #41 bring-up check. [`FLASH_COMMAND_FLASH_SD`]
//! writes the staged payload files into the existing FAT root, then the sealed
//! SD/SPI loader to sector 16, and reads both back. It never writes the MBR and
//! refuses a card whose FAT partition starts at or inside sector 16, so a failed
//! job leaves the boot record and partition in place (ADR-0014).

use core::arch::asm;
use core::slice;

use rawboot_image::{
    FLASH_ARGS_ADDR, FLASH_COMMAND_FLASH_SD, FLASH_COMMAND_NOOP, FLASH_COMMAND_RAW_WRITE_TEST,
    FLASH_JOB_ADDR, FLASH_JOB_SIZE, FLASH_LOADER_ADDR, FLASH_LOADER_START_LBA, FLASH_PAYLOAD_ADDR,
    FLASH_PLAN_ADDR, FLASH_PLAN_SIZE, FLASH_RESULT_BAD_JOB, FLASH_RESULT_BAD_PLAN,
    FLASH_RESULT_CARD_TOO_SMALL, FLASH_RESULT_FILE_CREATE, FLASH_RESULT_FILE_FLUSH,
    FLASH_RESULT_FILE_TRUNCATE, FLASH_RESULT_FILE_WRITE, FLASH_RESULT_MBR_CHANGED,
    FLASH_RESULT_NO_FAT, FLASH_RESULT_NO_MBR, FLASH_RESULT_OK, FLASH_RESULT_READ_FAILED,
    FLASH_RESULT_SD_OFFLINE, FLASH_RESULT_UNKNOWN_COMMAND, FLASH_RESULT_UNSAFE_LAYOUT,
    FLASH_RESULT_VERIFY_FAILED, FLASH_RESULT_WRITE_FAILED, FLASH_SECTOR_SIZE, FLASH_STATUS_ADDR,
    FLASH_STEP_DONE, FLASH_STEP_LAYOUT, FLASH_STEP_PLAN, FLASH_STEP_RAW_VERIFY,
    FLASH_STEP_RAW_WRITE, FLASH_STEP_READ_MBR, FLASH_STEP_SD_INIT, FLASH_STEP_VERIFY_FAT,
    FLASH_STEP_VERIFY_LOADER, FLASH_STEP_WRITE_ARGS, FLASH_STEP_WRITE_LOADER,
    FLASH_STEP_WRITE_PAYLOAD, FlashJob, FlashLayoutError, FlashPlan, FlashStatus, check_sd_layout,
    verify_sealed,
};
use sunxi_drivers::fat::{self, ARGS_FILE, PAYLOAD_FILE, block::SdCardIo, partition};
use sunxi_drivers::sdcard::{Sdcard_data, sdmmc_blk_read, sdmmc_blk_write, sdmmc_init};
use sunxi_drivers::sdhci::{self, Sdhci};
use sunxi_drivers::{clk, console, uart};
use sunxi_pl::io::{read8, write8};

const SEQUENCE_OFFSET: u32 = core::mem::offset_of!(FlashJob, sequence) as u32;
const MBR_SIZE: usize = partition::BLOCK_SIZE as usize;

struct Staged {
    plan: FlashPlan,
    payload: &'static [u8],
    args: &'static [u8],
    loader: &'static [u8],
}

/// Run the job currently in DRAM, then publish its status and return.
pub fn run() {
    let job = match read_job() {
        Ok(job) => job,
        Err(_) => {
            // The header is untrustworthy, but the host still needs the status:
            // echo the raw sequence word so a valid host job can match it.
            write_status(raw_sequence(), FLASH_STEP_DONE, FLASH_RESULT_BAD_JOB);
            return;
        }
    };

    let (step, result) = match job.command {
        FLASH_COMMAND_NOOP => (FLASH_STEP_DONE, FLASH_RESULT_OK),
        FLASH_COMMAND_FLASH_SD => run_flash_sd(),
        FLASH_COMMAND_RAW_WRITE_TEST => run_raw_write_test(),
        _ => (FLASH_STEP_DONE, FLASH_RESULT_UNKNOWN_COMMAND),
    };
    write_status(job.sequence, step, result);
}

/// Write `payload.bin`, the optional `boot.cfg` and the loader; report the
/// failing step and reason instead of returning an opaque error.
fn run_flash_sd() -> (u32, u32) {
    let staged = match read_plan() {
        Ok(staged) => staged,
        Err(status) => return status,
    };

    let mut sd = match init_card() {
        Ok(sd) => sd,
        Err(status) => return status,
    };

    let mut mbr = [0u8; MBR_SIZE];
    if read_block(&mut sd, 0, &mut mbr).is_err() {
        return (FLASH_STEP_READ_MBR, FLASH_RESULT_READ_FAILED);
    }
    if mbr[partition::SIGNATURE_OFFSET..] != partition::BOOT_SIGNATURE {
        return (FLASH_STEP_LAYOUT, FLASH_RESULT_NO_MBR);
    }

    let partitions = partition::fat_partitions(&mbr);
    if partitions.iter().flatten().next().is_none() {
        return if partition::bare_fat_partition(&mbr, sd.card.capacity).is_some() {
            (FLASH_STEP_LAYOUT, FLASH_RESULT_NO_MBR)
        } else {
            (FLASH_STEP_LAYOUT, FLASH_RESULT_NO_FAT)
        };
    }

    let mut outcome = None;
    for entry in partitions.into_iter().flatten() {
        // Check the MBR's declared extent before clamping it to the card.
        let declared = partition::Partition {
            base_lba: entry.base_lba,
            len_bytes: entry.len_bytes(),
        };
        if let Err(status) = check_layout(&sd, declared, staged.plan.loader_len) {
            return status;
        }
        let Some(candidate) = entry.partition(sd.card.capacity) else {
            return (FLASH_STEP_LAYOUT, FLASH_RESULT_CARD_TOO_SMALL);
        };

        match fat::mount_partition(SdCardIo::new(&mut sd), candidate) {
            Ok(fs) => {
                outcome = Some(flash_fat_files(
                    fs,
                    staged.plan,
                    staged.payload,
                    staged.args,
                ));
                break;
            }
            Err(_) => continue,
        }
    }

    match outcome {
        Some((FLASH_STEP_DONE, FLASH_RESULT_OK)) => {}
        Some(status) => return status,
        None => return (FLASH_STEP_LAYOUT, FLASH_RESULT_NO_FAT),
    }

    write_loader(&mut sd, &mbr, staged.loader)
}

/// Write a known block to the loader sector and read it back.
///
/// This is the smallest check of the SD block-write path: no MBR or FAT is
/// needed, and sector 0 is never touched. It is destructive only for sector 16,
/// which holds the SD loader on a prepared card.
fn run_raw_write_test() -> (u32, u32) {
    let mut sd = match init_card() {
        Ok(sd) => sd,
        Err(status) => return status,
    };

    let mut written = [0u8; RAW_TEST_BLOCK_SIZE];
    for (index, byte) in written.iter_mut().enumerate() {
        *byte = (index as u8)
            .wrapping_mul(RAW_TEST_MULTIPLIER)
            .wrapping_add(RAW_TEST_SEED);
    }
    written[..RAW_TEST_MAGIC.len()].copy_from_slice(RAW_TEST_MAGIC);

    console::print(format_args!(
        "FEL: raw write test lba {} blocks 1\r\n",
        FLASH_LOADER_START_LBA
    ));
    if sdmmc_blk_write(&mut sd, written.as_ptr(), FLASH_LOADER_START_LBA, 1).is_err() {
        return (FLASH_STEP_RAW_WRITE, FLASH_RESULT_WRITE_FAILED);
    }

    let mut read_back = [0u8; RAW_TEST_BLOCK_SIZE];
    if sdmmc_blk_read(&mut sd, read_back.as_mut_ptr(), FLASH_LOADER_START_LBA, 1) != 1 {
        return (FLASH_STEP_RAW_VERIFY, FLASH_RESULT_READ_FAILED);
    }
    if read_back != written {
        return (FLASH_STEP_RAW_VERIFY, FLASH_RESULT_VERIFY_FAILED);
    }

    console::print(format_args!("FEL: raw write test ok\r\n"));
    (FLASH_STEP_DONE, FLASH_RESULT_OK)
}

const RAW_TEST_BLOCK_SIZE: usize = FLASH_SECTOR_SIZE as usize;
const RAW_TEST_MAGIC: &[u8] = b"RBRW";
const RAW_TEST_MULTIPLIER: u8 = 37;
const RAW_TEST_SEED: u8 = 0x5a;

/// Read and validate the plan and the three staged regions.
fn read_plan() -> Result<Staged, (u32, u32)> {
    let mut bytes = [0u8; FLASH_PLAN_SIZE];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = read8(FLASH_PLAN_ADDR + index as u32);
    }
    let plan =
        FlashPlan::from_bytes(&bytes).map_err(|_| (FLASH_STEP_PLAN, FLASH_RESULT_BAD_PLAN))?;

    let payload = staged_region(FLASH_PAYLOAD_ADDR, plan.payload_len);
    let args = staged_region(FLASH_ARGS_ADDR, plan.args_len);
    let loader = staged_region(FLASH_LOADER_ADDR, plan.loader_len);
    if !plan.matches(payload, args, loader) {
        return Err((FLASH_STEP_PLAN, FLASH_RESULT_BAD_PLAN));
    }
    if verify_sealed(loader).is_err() {
        return Err((FLASH_STEP_PLAN, FLASH_RESULT_BAD_PLAN));
    }

    Ok(Staged {
        plan,
        payload,
        args,
        loader,
    })
}

/// A view of one staged region in DRAM.
fn staged_region(address: u32, length: u32) -> &'static [u8] {
    // SAFETY: the host wrote the region at this fixed DRAM address before it
    // executed the flasher, and the plan has been checked against its window.
    unsafe { slice::from_raw_parts(address as *const u8, length as usize) }
}

/// Bring up the SDHCI controller and the card.
fn init_card() -> Result<Sdcard_data, (u32, u32)> {
    clk::sunxi_clk_init();
    uart::sunxi_usart_init(&uart::usart0_dbg, 115200);
    console::set_console(&uart::UART_CONSOLE);

    let mut hci = Sdhci::default();
    sdhci::sunxi_sdhci_init(&mut hci);
    let mut sd = Sdcard_data::new(hci);

    if sdmmc_init(&mut sd) != 0 {
        return Err((FLASH_STEP_SD_INIT, FLASH_RESULT_SD_OFFLINE));
    }
    console::print(format_args!(
        "FEL: card version {:#x}, tran_speed {}, clock {}, hc {}, rbl {}, wbl {}, cap {}\r\n",
        sd.card.version,
        sd.card.tran_speed,
        sd.hci.clock_active,
        sd.card.high_capacity,
        sd.card.read_bl_len,
        sd.card.write_bl_len,
        sd.card.capacity
    ));
    Ok(sd)
}

/// Reject a partition whose start or capacity would put the loader outside the
/// safe prefix between the MBR and the partition.
fn check_layout(
    sd: &Sdcard_data,
    candidate: partition::Partition,
    loader_len: u32,
) -> Result<(), (u32, u32)> {
    check_sd_layout(
        candidate.base_lba,
        candidate.len_bytes,
        loader_len,
        sd.card.capacity,
    )
    .map_err(|error| match error {
        FlashLayoutError::PartitionBeyondCard => (FLASH_STEP_LAYOUT, FLASH_RESULT_CARD_TOO_SMALL),
        _ => (FLASH_STEP_LAYOUT, FLASH_RESULT_UNSAFE_LAYOUT),
    })
}

/// Replace the payload files, verify them, and unmount the volume.
fn flash_fat_files(
    fs: fatfs::FileSystem<fat::block::BlockDevice<SdCardIo<'_>>>,
    plan: FlashPlan,
    payload: &[u8],
    args: &[u8],
) -> (u32, u32) {
    let outcome = replace_and_verify(&fs, plan, payload, args);
    let unmount = fs.unmount();

    if outcome != (FLASH_STEP_DONE, FLASH_RESULT_OK) {
        return outcome;
    }
    if unmount.is_err() {
        return (FLASH_STEP_VERIFY_FAT, FLASH_RESULT_WRITE_FAILED);
    }

    (FLASH_STEP_DONE, FLASH_RESULT_OK)
}

fn replace_and_verify(
    fs: &fatfs::FileSystem<fat::block::BlockDevice<SdCardIo<'_>>>,
    plan: FlashPlan,
    payload: &[u8],
    args: &[u8],
) -> (u32, u32) {
    if let Err(error) = fat::create_or_replace_file(fs, PAYLOAD_FILE, payload) {
        return (FLASH_STEP_WRITE_PAYLOAD, write_result(error));
    }
    if plan.has_args()
        && let Err(error) = fat::create_or_replace_file(fs, ARGS_FILE, args)
    {
        return (FLASH_STEP_WRITE_ARGS, write_result(error));
    }

    if !fat::file_matches(fs, PAYLOAD_FILE, payload).unwrap_or(false) {
        return (FLASH_STEP_VERIFY_FAT, FLASH_RESULT_VERIFY_FAILED);
    }
    if plan.has_args() && !fat::file_matches(fs, ARGS_FILE, args).unwrap_or(false) {
        return (FLASH_STEP_VERIFY_FAT, FLASH_RESULT_VERIFY_FAILED);
    }

    (FLASH_STEP_DONE, FLASH_RESULT_OK)
}

fn write_result(error: fat::WriteError) -> u32 {
    match error {
        fat::WriteError::Create(_) => FLASH_RESULT_FILE_CREATE,
        fat::WriteError::Truncate(_) => FLASH_RESULT_FILE_TRUNCATE,
        fat::WriteError::Write(_) => FLASH_RESULT_FILE_WRITE,
        fat::WriteError::Flush(_) => FLASH_RESULT_FILE_FLUSH,
    }
}

/// Write the loader at sector 16, read it back, and confirm the MBR is unchanged.
fn write_loader(sd: &mut Sdcard_data, mbr: &[u8; MBR_SIZE], loader: &[u8]) -> (u32, u32) {
    let blocks = loader.len() as u64 / FLASH_SECTOR_SIZE;
    if sdmmc_blk_write(sd, loader.as_ptr(), FLASH_LOADER_START_LBA, blocks).is_err() {
        return (FLASH_STEP_WRITE_LOADER, FLASH_RESULT_WRITE_FAILED);
    }

    let mut block = [0u8; MBR_SIZE];
    for index in 0..blocks {
        if read_block(sd, FLASH_LOADER_START_LBA + index, &mut block).is_err() {
            return (FLASH_STEP_VERIFY_LOADER, FLASH_RESULT_VERIFY_FAILED);
        }
        let start = index as usize * MBR_SIZE;
        if block[..] != loader[start..start + MBR_SIZE] {
            return (FLASH_STEP_VERIFY_LOADER, FLASH_RESULT_VERIFY_FAILED);
        }
    }

    let mut mbr_after = [0u8; MBR_SIZE];
    if read_block(sd, 0, &mut mbr_after).is_err() {
        return (FLASH_STEP_VERIFY_LOADER, FLASH_RESULT_VERIFY_FAILED);
    }
    if &mbr_after != mbr {
        return (FLASH_STEP_VERIFY_LOADER, FLASH_RESULT_MBR_CHANGED);
    }

    (FLASH_STEP_DONE, FLASH_RESULT_OK)
}

fn read_block(sd: &mut Sdcard_data, lba: u64, block: &mut [u8; MBR_SIZE]) -> Result<(), ()> {
    if sdmmc_blk_read(sd, block.as_mut_ptr(), lba, 1) == 1 {
        Ok(())
    } else {
        Err(())
    }
}

fn read_job() -> Result<FlashJob, rawboot_image::FlashContractError> {
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

fn write_status(sequence: u32, step: u32, result: u32) {
    let status = FlashStatus::new(sequence, step, result);
    for (index, byte) in status.to_bytes().iter().enumerate() {
        write8(FLASH_STATUS_ADDR + index as u32, *byte);
    }
    // The host reads through the BROM after this function returns; order the
    // status stores before the return.
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}
