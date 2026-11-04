// SPDX-License-Identifier: CC-BY-4.0

#![allow(
    clippy::upper_case_acronyms,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
use crate::console::info;

use core::cmp::min;

use sunxi_pl::cpu::{time_ms, udelay};

use crate::sdhci::{
    MMC_BUS_WIDTH_1, MMC_BUS_WIDTH_4, MMC_CLK_25M, MMC_CLK_50M, MMC_CLK_50M_DDR, MMC_CLK_400K,
    MMC_VDD_27_36, MMC_VDD_165_195, Sdhci, SdhciCmd, SdhciData, sdhci_reset, sdhci_set_clock,
    sdhci_set_width, sdhci_transfer,
};

pub const MMC_GO_IDLE_STATE: u32 = 0;
pub const MMC_SEND_OP_COND: u32 = 1;
pub const MMC_ALL_SEND_CID: u32 = 2;
pub const MMC_SET_RELATIVE_ADDR: u32 = 3;
pub const MMC_SET_DSR: u32 = 4;
pub const MMC_SWITCH: u32 = 6;
pub const MMC_SELECT_CARD: u32 = 7;
pub const MMC_SEND_EXT_CSD: u32 = 8;
pub const MMC_SEND_CSD: u32 = 9;
pub const MMC_SEND_CID: u32 = 10;
pub const MMC_READ_DAT_UNTIL_STOP: u32 = 11;
pub const MMC_STOP_TRANSMISSION: u32 = 12;
pub const MMC_SEND_STATUS: u32 = 13;
pub const MMC_GO_INACTIVE_STATE: u32 = 15;
pub const MMC_SPI_READ_OCR: u32 = 58;
pub const MMC_SPI_CRC_ON_OFF: u32 = 59;

pub const MMC_SET_BLOCKLEN: u32 = 16;
pub const MMC_READ_SINGLE_BLOCK: u32 = 17;
pub const MMC_READ_MULTIPLE_BLOCK: u32 = 18;

pub const MMC_WRITE_DAT_UNTIL_STOP: u32 = 20;

pub const MMC_SET_BLOCK_COUNT: u32 = 23;
pub const MMC_WRITE_SINGLE_BLOCK: u32 = 24;
pub const MMC_WRITE_MULTIPLE_BLOCK: u32 = 25;
pub const MMC_PROGRAM_CID: u32 = 26;
pub const MMC_PROGRAM_CSD: u32 = 27;

pub const MMC_ERASE_GROUP_START: u32 = 35;
pub const MMC_ERASE_GROUP_END: u32 = 36;
pub const MMC_ERASE: u32 = 38;

pub const MMC_SET_WRITE_PROT: u32 = 28;
pub const MMC_CLR_WRITE_PROT: u32 = 29;
pub const MMC_SEND_WRITE_PROT: u32 = 30;

pub const MMC_LOCK_UNLOCK: u32 = 42;

pub const MMC_APP_CMD: u32 = 55;
pub const MMC_GEN_CMD: u32 = 56;

pub const MMC_FAST_IO: u32 = 39;
pub const MMC_GO_IRQ_STATE: u32 = 40;

pub const SD_CMD_SEND_RELATIVE_ADDR: u32 = 3;
pub const SD_CMD_SWITCH_FUNC: u32 = 6;
pub const SD_CMD_SEND_IF_COND: u32 = 8;
pub const SD_CMD_APP_SET_BUS_WIDTH: u32 = 6;
pub const SD_CMD_ERASE_WR_BLK_START: u32 = 32;
pub const SD_CMD_ERASE_WR_BLK_END: u32 = 33;
pub const SD_CMD_APP_SEND_OP_COND: u32 = 41;
pub const SD_CMD_APP_SEND_SCR: u32 = 51;

pub const MMC_RSP_PRESENT: u32 = 1 << 0;
pub const MMC_RSP_136: u32 = 1 << 1;
pub const MMC_RSP_CRC: u32 = 1 << 2;
pub const MMC_RSP_BUSY: u32 = 1 << 3;
pub const MMC_RSP_OPCODE: u32 = 1 << 4;

pub const MMC_RSP_NONE: u32 = 0 << 24;
pub const MMC_RSP_R1: u32 = (1 << 24) | (MMC_RSP_PRESENT | MMC_RSP_CRC | MMC_RSP_OPCODE);
pub const MMC_RSP_R1B: u32 =
    (1 << 24) | (MMC_RSP_PRESENT | MMC_RSP_CRC | MMC_RSP_OPCODE | MMC_RSP_BUSY);
pub const MMC_RSP_R2: u32 = (2 << 24) | (MMC_RSP_PRESENT | MMC_RSP_136 | MMC_RSP_CRC);
pub const MMC_RSP_R3: u32 = (3 << 24) | MMC_RSP_PRESENT;
pub const MMC_RSP_R4: u32 = (4 << 24) | MMC_RSP_PRESENT;
pub const MMC_RSP_R5: u32 = (5 << 24) | (MMC_RSP_PRESENT | MMC_RSP_CRC | MMC_RSP_OPCODE);
pub const MMC_RSP_R6: u32 = (6 << 24) | (MMC_RSP_PRESENT | MMC_RSP_CRC | MMC_RSP_OPCODE);
pub const MMC_RSP_R7: u32 = (7 << 24) | (MMC_RSP_PRESENT | MMC_RSP_CRC | MMC_RSP_OPCODE);

pub const MMC_STATUS_IDLE: u32 = 0;
pub const MMC_STATUS_READY: u32 = 1;
pub const MMC_STATUS_IDENT: u32 = 2;
pub const MMC_STATUS_STBY: u32 = 3;
pub const MMC_STATUS_TRAN: u32 = 4;
pub const MMC_STATUS_DATA: u32 = 5;
pub const MMC_STATUS_RCV: u32 = 6;
pub const MMC_STATUS_PRG: u32 = 7;
pub const MMC_STATUS_DIS: u32 = 8;
pub const MMC_STATUS_BTST: u32 = 9;
pub const MMC_STATUS_SLP: u32 = 10;

pub const MMC_STATUS_ERROR: u32 = 1 << 19;
pub const MMC_STATUS_CC_ERROR: u32 = 1 << 20;
pub const MMC_STATUS_ECC_FAILED: u32 = 1 << 21;
pub const MMC_STATUS_ILLEGAL_COMMAND: u32 = 1 << 22;
pub const MMC_STATUS_COM_CRC_ERROR: u32 = 1 << 23;
pub const MMC_STATUS_LOCK_UNLOCK_FAILED: u32 = 1 << 24;
pub const MMC_STATUS_WP_VIOLATION: u32 = 1 << 26;
pub const MMC_STATUS_ERASE_PARAM: u32 = 1 << 27;
pub const MMC_STATUS_ERASE_SEQ_ERROR: u32 = 1 << 28;
pub const MMC_STATUS_BLOCK_LEN_ERROR: u32 = 1 << 29;
pub const MMC_STATUS_ADDRESS_ERROR: u32 = 1 << 30;
pub const MMC_STATUS_OUT_OF_RANGE: u32 = 1 << 31;
#[cfg(feature = "write")]
const MMC_STATUS_ERROR_BITS: u32 = MMC_STATUS_ERROR
    | MMC_STATUS_CC_ERROR
    | MMC_STATUS_ECC_FAILED
    | MMC_STATUS_ILLEGAL_COMMAND
    | MMC_STATUS_COM_CRC_ERROR
    | MMC_STATUS_LOCK_UNLOCK_FAILED
    | MMC_STATUS_WP_VIOLATION
    | MMC_STATUS_ERASE_PARAM
    | MMC_STATUS_ERASE_SEQ_ERROR
    | MMC_STATUS_BLOCK_LEN_ERROR
    | MMC_STATUS_ADDRESS_ERROR
    | MMC_STATUS_OUT_OF_RANGE;

pub const OCR_BUSY: u32 = 0x8000_0000;
pub const OCR_HCS: u32 = 0x4000_0000;
pub const OCR_VOLTAGE_MASK: u32 = 0x00ff_ff80;
pub const OCR_ACCESS_MODE: u32 = 0x6000_0000;

const SD_IF_COND_VHS_27_36: u32 = 0x1 << 8;
const SD_IF_COND_VHS_165_195: u32 = 0x2 << 8;
const SD_IF_COND_VHS_RESERVED: u32 = 0x0 << 8;
const SD_IF_COND_CHECK_PATTERN: u32 = 0xaa;
const SD_ACMD41_VDD_27_36: u32 = 0x00ff_8000;
const SD_ACMD41_VDD_165_195: u32 = 0x0000_0080;
const MMC_OCR_SECTOR_MODE_VDD_27_36: u32 = 0x40ff_8000;
const MMC_OCR_SECTOR_MODE_VDD_165_195: u32 = 0x40ff_0080;
const EXT_CSD_SWITCH_ACCESS_MODE: u32 = 3 << 24;

pub const MMC_DATA_READ: u32 = 1 << 0;
pub const MMC_DATA_WRITE: u32 = 1 << 1;

pub const SD_VERSION_SD: u32 = 0x20000;
pub const SD_VERSION_3: u32 = SD_VERSION_SD | 0x300;
pub const SD_VERSION_2: u32 = SD_VERSION_SD | 0x200;
pub const SD_VERSION_1_0: u32 = SD_VERSION_SD | 0x100;
pub const SD_VERSION_1_10: u32 = SD_VERSION_SD | 0x10a;
pub const MMC_VERSION_MMC: u32 = 0x10000;
pub const MMC_VERSION_UNKNOWN: u32 = MMC_VERSION_MMC;
pub const MMC_VERSION_1_2: u32 = MMC_VERSION_MMC | 0x102;
pub const MMC_VERSION_1_4: u32 = MMC_VERSION_MMC | 0x104;
pub const MMC_VERSION_2_2: u32 = MMC_VERSION_MMC | 0x202;
pub const MMC_VERSION_3: u32 = MMC_VERSION_MMC | 0x300;
pub const MMC_VERSION_4: u32 = MMC_VERSION_MMC | 0x400;
pub const MMC_VERSION_4_1: u32 = MMC_VERSION_MMC | 0x401;
pub const MMC_VERSION_4_2: u32 = MMC_VERSION_MMC | 0x402;
pub const MMC_VERSION_4_3: u32 = MMC_VERSION_MMC | 0x403;
pub const MMC_VERSION_4_41: u32 = MMC_VERSION_MMC | 0x429;
pub const MMC_VERSION_4_5: u32 = MMC_VERSION_MMC | 0x405;
pub const MMC_VERSION_5_0: u32 = MMC_VERSION_MMC | 0x500;
pub const MMC_VERSION_5_1: u32 = MMC_VERSION_MMC | 0x501;

pub const EXT_CSD_HS_TIMING: usize = 185;
pub const EXT_CSD_POWER_CLASS: usize = 187;
pub const EXT_CSD_REV: usize = 192;
pub const EXT_CSD_CARD_TYPE: usize = 196;
pub const EXT_CSD_PWR_CL_52_360: usize = 202;
pub const EXT_CSD_SEC_CNT: usize = 212;
pub const EXT_CSD_PWR_CL_DDR_52_360: usize = 239;
pub const EXT_CSD_BUS_WIDTH: usize = 183;

pub const EXT_CSD_BUS_WIDTH_1: u32 = 0;
pub const EXT_CSD_BUS_WIDTH_4: u32 = 1;
pub const EXT_CSD_BUS_WIDTH_8: u32 = 2;
pub const EXT_CSD_DDR_BUS_WIDTH_4: u32 = 5;
pub const EXT_CSD_DDR_BUS_WIDTH_8: u32 = 6;

pub const EXT_CSD_TIMING_BC: u32 = 0;
pub const EXT_CSD_TIMING_HS: u32 = 1;

pub const EXT_CSD_CARD_TYPE_26: u32 = 1 << 0;
pub const EXT_CSD_CARD_TYPE_52: u32 = 1 << 1;
pub const EXT_CSD_CARD_TYPE_MASK: u32 = 0x3f;
pub const EXT_CSD_CARD_TYPE_DDR_1_8V: u32 = 1 << 2;
pub const EXT_CSD_CARD_TYPE_DDR_1_2V: u32 = 1 << 3;
pub const EXT_CSD_CARD_TYPE_DDR_52: u32 = EXT_CSD_CARD_TYPE_DDR_1_8V | EXT_CSD_CARD_TYPE_DDR_1_2V;
pub const EXT_CSD_CARD_TYPE_SDR_1_8V: u32 = 1 << 4;
pub const EXT_CSD_CARD_TYPE_SDR_1_2V: u32 = 1 << 5;

pub const EXT_CSD_CMD_SET_NORMAL: u32 = 1 << 0;
pub const EXT_CSD_CMD_SET_SECURE: u32 = 1 << 1;
pub const EXT_CSD_CMD_SET_CPSECURE: u32 = 1 << 2;

pub const EXT_CSD_PWR_CL_8BIT_MASK: u8 = 0xF0;
pub const EXT_CSD_PWR_CL_4BIT_MASK: u8 = 0x0F;
pub const EXT_CSD_PWR_CL_8BIT_SHIFT: u32 = 4;
pub const EXT_CSD_PWR_CL_4BIT_SHIFT: u32 = 0;

const TRAN_SPEED_UNIT: [u32; 4] = [10000, 100000, 1000000, 10000000];
const TRAN_SPEED_TIME: [u8; 16] = [
    0, 10, 12, 13, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 70, 80,
];

#[derive(Debug, Clone, Copy)]
pub struct Sdcard {
    pub version: u32,
    pub ocr: u32,
    pub rca: u32,
    pub cid: [u32; 4],
    pub csd: [u32; 4],
    pub extcsd: [u8; 512],
    pub high_capacity: u32,
    pub tran_speed: u32,
    pub dsr_imp: u32,
    pub read_bl_len: u32,
    pub write_bl_len: u32,
    pub capacity: u64,
}

pub struct Sdcard_data {
    pub card: Sdcard,
    pub hci: Sdhci,
    pub buf: [u8; 512],
    pub online: bool,
}

impl Sdcard_data {
    /// A fresh, offline card context for `hci`.
    pub fn new(hci: Sdhci) -> Self {
        Self {
            card: Sdcard {
                version: 0,
                ocr: 0,
                rca: 0,
                cid: [0u32; 4],
                csd: [0u32; 4],
                extcsd: [0u8; 512],
                high_capacity: 0,
                tran_speed: 0,
                dsr_imp: 0,
                read_bl_len: 0,
                write_bl_len: 0,
                capacity: 0,
            },
            hci,
            buf: [0u8; 512],
            online: false,
        }
    }
}

/// Why a card write was rejected or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdError {
    /// The card did not initialise, so nothing may be written to it.
    Offline,
    /// A block write failed; the range is reported for the caller's log.
    Write { blkno: u64, blkcnt: u64 },
}

pub fn sdmmc_init(data: &mut Sdcard_data) -> i32 {
    data.online = false;
    let mut retries = 10i32;

    loop {
        if sdmmc_detect(&mut data.hci, &mut data.card) {
            data.online = true;
            info!("SD/MMC card found\r\n");
            return 0;
        }

        udelay(100_000);
        let retries_left = retries > 0;
        retries -= 1;
        if !retries_left {
            break;
        }
    }

    info!("SD/MMC card not found\r\n");
    -1
}

pub fn sdmmc_blk_read(
    data: &mut Sdcard_data,
    mut buf: *mut u8,
    mut blkno: u64,
    blkcnt: u64,
) -> u64 {
    let mut blks = blkcnt;
    let card = data.card;

    while blks > 0 {
        let cnt = if blks > 127 { 127 } else { blks };
        if sdmmc_read_blocks(&mut data.hci, &card, buf, blkno, cnt) != cnt {
            return 0;
        }
        blks -= cnt;
        blkno += cnt;
        buf = buf.wrapping_add((cnt * card.read_bl_len as u64) as usize);
    }
    blkcnt
}

/// Writes `blkcnt` blocks starting at `blkno` from `buf`.
///
/// The caller owns `buf`; the card never reads past `blkcnt * blksz` bytes. The
/// write is split into the same 127-block chunks as [`sdmmc_blk_read`]. An
/// offline card and a failed transfer are reported instead of being treated as
/// a successful short write.
#[cfg(feature = "write")]
pub fn sdmmc_blk_write(
    data: &mut Sdcard_data,
    mut buf: *const u8,
    mut blkno: u64,
    blkcnt: u64,
) -> Result<(), SdError> {
    if !data.online {
        return Err(SdError::Offline);
    }

    let mut blks = blkcnt;
    let card = data.card;

    while blks > 0 {
        let cnt = if blks > 127 { 127 } else { blks };
        if sdmmc_write_blocks(&mut data.hci, &card, buf, blkno, cnt) != cnt {
            return Err(SdError::Write { blkno, blkcnt: cnt });
        }
        blks -= cnt;
        blkno += cnt;
        buf = buf.wrapping_add((cnt * card.write_bl_len as u64) as usize);
    }
    Ok(())
}

fn go_idle_state(hci: &mut Sdhci) -> bool {
    let mut cmd = SdhciCmd {
        idx: MMC_GO_IDLE_STATE,
        resptype: if hci.isspi { MMC_RSP_R1 } else { MMC_RSP_NONE },
        ..Default::default()
    };

    sdhci_transfer(hci, &mut cmd, None)
}

fn sd_send_if_cond(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd {
        idx: SD_CMD_SEND_IF_COND,
        arg: if (hci.voltage & MMC_VDD_27_36) != 0 {
            SD_IF_COND_VHS_27_36
        } else if (hci.voltage & MMC_VDD_165_195) != 0 {
            SD_IF_COND_VHS_165_195
        } else {
            SD_IF_COND_VHS_RESERVED
        } | SD_IF_COND_CHECK_PATTERN,
        resptype: MMC_RSP_R7,
        ..Default::default()
    };
    if !sdhci_transfer(hci, &mut cmd, None) {
        return false;
    }

    if (cmd.response[0] & 0xff) != 0xaa {
        return false;
    }
    card.version = SD_VERSION_2;
    true
}

fn sd_send_op_cond(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd::default();
    let mut retries = 50i32;

    if !go_idle_state(hci) {
        return false;
    }

    if !sd_send_if_cond(hci, card) {
        return false;
    }

    loop {
        cmd.idx = MMC_APP_CMD;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R1;
        if sdhci_transfer(hci, &mut cmd, None) {
            cmd.idx = SD_CMD_APP_SEND_OP_COND;
            if hci.isspi {
                cmd.arg = 0;
                if card.version == SD_VERSION_2 {
                    cmd.arg |= OCR_HCS;
                }
                cmd.resptype = MMC_RSP_R1;
                if sdhci_transfer(hci, &mut cmd, None) {
                    break;
                }
            } else {
                if (hci.voltage & MMC_VDD_27_36) != 0 {
                    cmd.arg = SD_ACMD41_VDD_27_36;
                } else if (hci.voltage & MMC_VDD_165_195) != 0 {
                    cmd.arg = SD_ACMD41_VDD_165_195;
                } else {
                    cmd.arg = 0;
                }
                if card.version == SD_VERSION_2 {
                    cmd.arg |= OCR_HCS;
                }
                cmd.resptype = MMC_RSP_R3;
                if !sdhci_transfer(hci, &mut cmd, None) || (cmd.response[0] & OCR_BUSY) != 0 {
                    break;
                }
            }
        }

        let retries_left = retries > 0;
        retries -= 1;
        if !retries_left {
            break;
        }
    }

    if retries <= 0 {
        return false;
    }

    if card.version != SD_VERSION_2 {
        card.version = SD_VERSION_1_0;
    }
    if hci.isspi {
        cmd.idx = MMC_SPI_READ_OCR;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R3;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
    }
    card.ocr = cmd.response[0];
    card.high_capacity = ((card.ocr & OCR_HCS) == OCR_HCS) as u32;
    card.rca = 0;

    true
}

fn mmc_send_op_cond(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd::default();
    let mut retries = 50i32;

    cmd.idx = MMC_SEND_OP_COND;
    cmd.resptype = MMC_RSP_R3;
    if hci.voltage == MMC_VDD_27_36 {
        cmd.arg = MMC_OCR_SECTOR_MODE_VDD_27_36;
    } else if hci.voltage == MMC_VDD_165_195 {
        cmd.arg = MMC_OCR_SECTOR_MODE_VDD_165_195;
    }

    loop {
        cmd.response[0] = 0;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        udelay(5000);

        if (cmd.response[0] & OCR_BUSY) != 0 {
            break;
        }
        let retries_left = retries > 0;
        retries -= 1;
        if !retries_left {
            break;
        }
    }

    if retries <= 0 {
        return false;
    }

    if hci.isspi {
        cmd.idx = MMC_SPI_READ_OCR;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R3;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
    }
    card.version = MMC_VERSION_UNKNOWN;
    card.ocr = cmd.response[0];
    card.high_capacity = ((card.ocr & OCR_HCS) == OCR_HCS) as u32;
    card.rca = 1;
    true
}

/// How long to wait for the card to leave its programming state.
///
/// A single-block write can keep `READY_FOR_DATA` clear for milliseconds; a
/// read returns it immediately. Polling for only a few hundred microseconds
/// turns a successful write into a reported failure and makes the next command
/// race the card while it is still programming.
const MMC_STATUS_TIMEOUT_MS: u32 = 1000;
const MMC_STATUS_POLL_US: u64 = 100;
const MMC_STATUS_ERROR_RETRIES: u32 = 10;

fn sdmmc_status_response(hci: &mut Sdhci, card: &Sdcard) -> Option<u32> {
    let mut cmd = SdhciCmd::default();
    let start = time_ms();
    let mut failures = 0;

    cmd.idx = MMC_SEND_STATUS;
    cmd.resptype = MMC_RSP_R1;
    cmd.arg = card.rca << 16;
    loop {
        if sdhci_transfer(hci, &mut cmd, None) {
            failures = 0;
            if (cmd.response[0] & (1 << 8)) != 0 {
                return Some(cmd.response[0]);
            }
        } else {
            // One missed poll can be transient, so retry a few times before
            // giving up; a truly unresponsive card stops after ten timeouts
            // instead of flooding the log for the whole deadline.
            failures += 1;
            if failures >= MMC_STATUS_ERROR_RETRIES {
                info!("SMHC: status error\r\n");
                return None;
            }
        }
        if time_ms().wrapping_sub(start) > MMC_STATUS_TIMEOUT_MS {
            info!("SMHC: status error\r\n");
            return None;
        }
        udelay(MMC_STATUS_POLL_US);
    }
}

fn sdmmc_status(hci: &mut Sdhci, card: &Sdcard) -> i32 {
    match sdmmc_status_response(hci, card) {
        Some(response) => ((response >> 9) & 0xf) as i32,
        None => -1,
    }
}

fn sdmmc_read_blocks(hci: &mut Sdhci, card: &Sdcard, buf: *mut u8, start: u64, blkcnt: u64) -> u64 {
    let mut cmd = SdhciCmd::default();
    let dat = SdhciData {
        buf,
        flag: MMC_DATA_READ,
        blksz: card.read_bl_len,
        blkcnt: blkcnt as u32,
    };
    let mut status;

    if blkcnt > 1 {
        cmd.idx = MMC_READ_MULTIPLE_BLOCK;
    } else {
        cmd.idx = MMC_READ_SINGLE_BLOCK;
    }
    if card.high_capacity != 0 {
        cmd.arg = start as u32;
    } else {
        cmd.arg = (start * card.read_bl_len as u64) as u32;
    }
    cmd.resptype = MMC_RSP_R1;

    if !sdhci_transfer(hci, &mut cmd, Some(&dat)) {
        info!("SMHC: read failed\r\n");
        return 0;
    }

    if !hci.isspi {
        loop {
            status = sdmmc_status(hci, card);
            if status < 0 {
                return 0;
            }

            if status == MMC_STATUS_TRAN as i32 || status == MMC_STATUS_DATA as i32 {
                break;
            }
        }
    }

    if blkcnt > 1 {
        cmd.idx = MMC_STOP_TRANSMISSION;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R1B;
        if !sdhci_transfer(hci, &mut cmd, None) {
            info!("SMHC: transfer stop failed\r\n");
            return 0;
        }
    }
    blkcnt
}

#[cfg(feature = "write")]
fn sdmmc_write_blocks(
    hci: &mut Sdhci,
    card: &Sdcard,
    buf: *const u8,
    start: u64,
    blkcnt: u64,
) -> u64 {
    let mut cmd = SdhciCmd::default();
    let dat = SdhciData {
        buf: buf as *mut u8,
        flag: MMC_DATA_WRITE,
        blksz: card.write_bl_len,
        blkcnt: blkcnt as u32,
    };
    if blkcnt > 1 {
        cmd.idx = MMC_WRITE_MULTIPLE_BLOCK;
    } else {
        cmd.idx = MMC_WRITE_SINGLE_BLOCK;
    }
    if card.high_capacity != 0 {
        cmd.arg = start as u32;
    } else {
        cmd.arg = (start * card.write_bl_len as u64) as u32;
    }
    cmd.resptype = MMC_RSP_R1;

    if !sdhci_transfer(hci, &mut cmd, Some(&dat)) {
        info!("SMHC: write failed\r\n");
        return 0;
    }

    if !hci.isspi {
        loop {
            let Some(response) = sdmmc_status_response(hci, card) else {
                return 0;
            };
            if response & MMC_STATUS_ERROR_BITS != 0 {
                info!("SMHC: write status error 0x{:08x}\r\n", response);
                return 0;
            }

            if (response >> 9) & 0xf == MMC_STATUS_TRAN {
                break;
            }
        }
    }

    if blkcnt > 1 {
        cmd.idx = MMC_STOP_TRANSMISSION;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R1B;
        if !sdhci_transfer(hci, &mut cmd, None) {
            info!("SMHC: transfer stop failed\r\n");
            return 0;
        }
    }
    blkcnt
}

fn sdmmc_detect(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd::default();
    let csize: u64;
    let cmult: u64;
    let width: u32;
    let mut status: i32;

    sdhci_reset(hci);
    if !sdhci_set_clock(hci, MMC_CLK_400K) || !sdhci_set_width(hci, MMC_BUS_WIDTH_1) {
        info!("SMHC: set clock/width failed\r\n");
        return false;
    }

    if !go_idle_state(hci) {
        info!("SMHC: set idle state failed\r\n");
        return false;
    }
    udelay(2000);

    if !sd_send_op_cond(hci, card) {
        sdhci_reset(hci);
        sdhci_set_clock(hci, MMC_CLK_400K);
        sdhci_set_width(hci, MMC_BUS_WIDTH_1);

        if !go_idle_state(hci) {
            info!("SMHC: set idle state failed\r\n");
            return false;
        }
        udelay(2000);

        if !mmc_send_op_cond(hci, card) {
            return false;
        }
    }

    if hci.isspi {
        cmd.idx = MMC_SEND_CID;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R1;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.cid[0] = cmd.response[0];
        card.cid[1] = cmd.response[1];
        card.cid[2] = cmd.response[2];
        card.cid[3] = cmd.response[3];

        cmd.idx = MMC_SEND_CSD;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R1;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.csd[0] = cmd.response[0];
        card.csd[1] = cmd.response[1];
        card.csd[2] = cmd.response[2];
        card.csd[3] = cmd.response[3];
    } else {
        cmd.idx = MMC_ALL_SEND_CID;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R2;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.cid[0] = cmd.response[0];
        card.cid[1] = cmd.response[1];
        card.cid[2] = cmd.response[2];
        card.cid[3] = cmd.response[3];

        cmd.idx = SD_CMD_SEND_RELATIVE_ADDR;
        cmd.arg = card.rca << 16;
        cmd.resptype = MMC_RSP_R6;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        if (card.version & SD_VERSION_SD) != 0 {
            card.rca = (cmd.response[0] >> 16) & 0xffff;
        }

        cmd.idx = MMC_SEND_CSD;
        cmd.arg = card.rca << 16;
        cmd.resptype = MMC_RSP_R2;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.csd[0] = cmd.response[0];
        card.csd[1] = cmd.response[1];
        card.csd[2] = cmd.response[2];
        card.csd[3] = cmd.response[3];

        cmd.idx = MMC_SELECT_CARD;
        cmd.arg = card.rca << 16;
        cmd.resptype = MMC_RSP_R1;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        loop {
            status = sdmmc_status(hci, card);
            if status < 0 {
                return false;
            }
            if status == MMC_STATUS_TRAN as i32 {
                break;
            }
        }
    }

    if card.version == MMC_VERSION_UNKNOWN {
        card.version = match (card.csd[0] >> 26) & 0xf {
            0 => MMC_VERSION_1_2,
            1 => MMC_VERSION_1_4,
            2 => MMC_VERSION_2_2,
            3 => MMC_VERSION_3,
            4 => MMC_VERSION_4,
            _ => MMC_VERSION_1_2,
        };
    }

    let unit: u32 = TRAN_SPEED_UNIT[(card.csd[0] & 0x7) as usize];
    let time: u32 = TRAN_SPEED_TIME[((card.csd[0] >> 3) & 0xf) as usize] as u32;
    card.tran_speed = time * unit;
    card.dsr_imp = UNSTUFF_BITS(card.csd, 76, 1) as u32;

    card.read_bl_len = 1u32 << UNSTUFF_BITS(card.csd, 80, 4);
    if (card.version & SD_VERSION_SD) != 0 {
        card.write_bl_len = card.read_bl_len;
    } else {
        card.write_bl_len = 1 << ((card.csd[3] >> 22) & 0xf);
    }
    if card.read_bl_len > 512 {
        card.read_bl_len = 512;
    }
    if card.write_bl_len > 512 {
        card.write_bl_len = 512;
    }

    if (card.version & MMC_VERSION_MMC) != 0 && card.version >= MMC_VERSION_4 {
        card.tran_speed = 50_000_000;
        cmd.idx = MMC_SEND_EXT_CSD;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R1;
        let dat = SdhciData {
            buf: card.extcsd.as_mut_ptr(),
            flag: MMC_DATA_READ,
            blksz: 512,
            blkcnt: 1,
        };
        if !sdhci_transfer(hci, &mut cmd, Some(&dat)) {
            return false;
        }
        if !hci.isspi {
            loop {
                status = sdmmc_status(hci, card);
                if status < 0 {
                    return false;
                }
                if status == MMC_STATUS_TRAN as i32 {
                    break;
                }
            }
        }
        card.version = match card.extcsd[EXT_CSD_REV] {
            1 => MMC_VERSION_4_1,
            2 => MMC_VERSION_4_2,
            3 => MMC_VERSION_4_3,
            5 => MMC_VERSION_4_41,
            6 => MMC_VERSION_4_5,
            7 => MMC_VERSION_5_0,
            8 => MMC_VERSION_5_1,
            _ => card.version,
        };
    }

    if card.high_capacity != 0 {
        if (card.version & SD_VERSION_SD) != 0 {
            csize = UNSTUFF_BITS(card.csd, 48, 22);
            card.capacity = (1 + csize) << 10;
        } else {
            card.capacity = card.extcsd[EXT_CSD_SEC_CNT] as u64
                | (card.extcsd[EXT_CSD_SEC_CNT + 1] as u64) << 8
                | (card.extcsd[EXT_CSD_SEC_CNT + 2] as u64) << 16
                | (card.extcsd[EXT_CSD_SEC_CNT + 3] as u64) << 24;
        }
    } else {
        cmult = UNSTUFF_BITS(card.csd, 47, 3);
        csize = UNSTUFF_BITS(card.csd, 62, 12);
        card.capacity = (csize + 1) << (cmult + 2);
    }
    card.capacity *= 1 << UNSTUFF_BITS(card.csd, 80, 4);

    if hci.isspi {
        if !sdhci_set_clock(hci, min(card.tran_speed, hci.clock_wanted))
            || !sdhci_set_width(hci, MMC_BUS_WIDTH_1)
        {
            info!("SMHC: set clock/width failed\r\n");
            return false;
        }
    } else {
        if (card.version & SD_VERSION_SD) != 0 {
            if hci.width == MMC_BUS_WIDTH_4 {
                width = 2;
            } else {
                width = 0;
            }

            cmd.idx = MMC_APP_CMD;
            cmd.arg = card.rca << 16;
            cmd.resptype = MMC_RSP_R5;
            if !sdhci_transfer(hci, &mut cmd, None) {
                return false;
            }

            cmd.idx = SD_CMD_SWITCH_FUNC;
            cmd.arg = width;
            cmd.resptype = MMC_RSP_R1;
            if !sdhci_transfer(hci, &mut cmd, None) {
                return false;
            }
        } else if (card.version & MMC_VERSION_MMC) != 0 {
            let card_type = card.extcsd[EXT_CSD_CARD_TYPE] as u32;
            let mut want_ddr = hci.clock_wanted == MMC_CLK_50M_DDR;

            if want_ddr && (card_type & EXT_CSD_CARD_TYPE_DDR_52) == 0 {
                info!(
                    "SMHC: controller requested DDR but card does not advertise DDR support, falling back to high-speed SDR\r\n"
                );
                hci.clock_wanted = MMC_CLK_50M;
                want_ddr = false;
            }

            if hci.clock_wanted >= MMC_CLK_50M {
                if (card_type & (EXT_CSD_CARD_TYPE_52 | EXT_CSD_CARD_TYPE_DDR_52)) == 0 {
                    info!("SMHC: card does not advertise high-speed timing, using 25MHz\r\n");
                    hci.clock_wanted = MMC_CLK_25M;
                    want_ddr = false;
                } else {
                    cmd.idx = SD_CMD_SWITCH_FUNC;
                    cmd.resptype = MMC_RSP_R1;
                    cmd.arg = EXT_CSD_SWITCH_ACCESS_MODE
                        | ((EXT_CSD_HS_TIMING as u32) << 16)
                        | (EXT_CSD_TIMING_HS << 8)
                        | EXT_CSD_CMD_SET_NORMAL;
                    if !sdhci_transfer(hci, &mut cmd, None) {
                        return false;
                    }
                    udelay(1000);
                    card.extcsd[EXT_CSD_HS_TIMING] = EXT_CSD_TIMING_HS as u8;
                }
            }

            width = match hci.width {
                MMC_BUS_WIDTH_4 => {
                    if want_ddr {
                        EXT_CSD_DDR_BUS_WIDTH_4
                    } else {
                        EXT_CSD_BUS_WIDTH_4
                    }
                }
                _ => EXT_CSD_BUS_WIDTH_1,
            };

            if hci.width >= MMC_BUS_WIDTH_4 {
                cmd.idx = SD_CMD_SWITCH_FUNC;
                cmd.resptype = MMC_RSP_R1;
                cmd.arg = EXT_CSD_SWITCH_ACCESS_MODE
                    | ((EXT_CSD_POWER_CLASS as u32) << 16)
                    | EXT_CSD_CMD_SET_NORMAL;
                let pwr = if width == EXT_CSD_DDR_BUS_WIDTH_4 {
                    card.extcsd[EXT_CSD_PWR_CL_DDR_52_360]
                } else {
                    card.extcsd[EXT_CSD_PWR_CL_52_360]
                };
                cmd.arg |=
                    ((pwr & (EXT_CSD_PWR_CL_4BIT_MASK >> EXT_CSD_PWR_CL_4BIT_SHIFT)) as u32) << 8;

                if !sdhci_transfer(hci, &mut cmd, None) {
                    return false;
                }
            }

            cmd.idx = SD_CMD_SWITCH_FUNC;
            cmd.resptype = MMC_RSP_R1;
            cmd.arg = EXT_CSD_SWITCH_ACCESS_MODE
                | ((EXT_CSD_BUS_WIDTH as u32) << 16)
                | (width << 8)
                | EXT_CSD_CMD_SET_NORMAL;
            if !sdhci_transfer(hci, &mut cmd, None) {
                return false;
            }

            udelay(1000);
        }

        if !sdhci_set_clock(hci, hci.clock_wanted) || !sdhci_set_width(hci, hci.width) {
            info!("SMHC: set clock/width failed\r\n");
            return false;
        }
    }

    cmd.idx = MMC_SET_BLOCKLEN;
    cmd.arg = card.read_bl_len;
    cmd.resptype = MMC_RSP_R1;
    if !sdhci_transfer(hci, &mut cmd, None) {
        return false;
    }

    true
}

fn UNSTUFF_BITS(resp: [u32; 4], start: u32, size: u32) -> u64 {
    let __size = size;
    let __mask: u32 = if __size < 32 { 1 << __size } else { 0 } - 1;
    let __off = (3 - (start / 32)) as usize;
    let __shft = start & 31;
    let mut __res;

    __res = resp[__off] >> __shft;
    if __size + __shft > 32 {
        __res |= resp[__off - 1] << ((32 - __shft) % 32);
    }
    (__res & __mask) as u64
}
