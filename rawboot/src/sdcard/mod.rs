// SPDX-License-Identifier: Unlicense

use core::cmp::min;
use rawboot_rt::cpu::udelay;

use crate::{
    print,
    sdhci::{
        MMC_BUS_WIDTH_1, MMC_BUS_WIDTH_4, MMC_CLK_400K, Sdhci, SdhciCmd, SdhciData, sdhci_reset,
        sdhci_set_clock, sdhci_set_width, sdhci_transfer,
    },
};

pub enum MMC {
    /* Class 1 */
    GO_IDLE_STATE = 0,
    SEND_OP_COND = 1,
    ALL_SEND_CID = 2,
    SET_RELATIVE_ADDR = 3,
    SET_DSR = 4,
    SWITCH = 6,
    SELECT_CARD = 7,
    SEND_EXT_CSD = 8,
    SEND_CSD = 9,
    SEND_CID = 10,
    READ_DAT_UNTIL_STOP = 11,
    STOP_TRANSMISSION = 12,
    SEND_STATUS = 13,
    GO_INACTIVE_STATE = 15,
    SPI_READ_OCR = 58,
    SPI_CRC_ON_OFF = 59,

    /* Class 2 */
    SET_BLOCKLEN = 16,
    READ_SINGLE_BLOCK = 17,
    READ_MULTIPLE_BLOCK = 18,

    /* Class 3 */
    WRITE_DAT_UNTIL_STOP = 20,

    /* Class 4 */
    SET_BLOCK_COUNT = 23,
    WRITE_SINGLE_BLOCK = 24,
    WRITE_MULTIPLE_BLOCK = 25,
    PROGRAM_CID = 26,
    PROGRAM_CSD = 27,

    /* Class 5 */
    ERASE_GROUP_START = 35,
    ERASE_GROUP_END = 36,
    ERASE = 38,

    /* Class 6 */
    SET_WRITE_PROT = 28,
    CLR_WRITE_PROT = 29,
    SEND_WRITE_PROT = 30,

    /* Class 7 */
    LOCK_UNLOCK = 42,

    /* Class 8 */
    APP_CMD = 55,
    GEN_CMD = 56,

    /* Class 9 */
    FAST_IO = 39,
    GO_IRQ_STATE = 40,
}

pub mod SD_Cmd {
    pub const SEND_RELATIVE_ADDR: u32 = 3;
    pub const SWITCH_FUNC: u32 = 6;
    pub const SEND_IF_COND: u32 = 8;
    pub const APP_SET_BUS_WIDTH: u32 = 6;
    pub const ERASE_WR_BLK_START: u32 = 32;
    pub const ERASE_WR_BLK_END: u32 = 33;
    pub const APP_SEND_OP_COND: u32 = 41;
    pub const APP_SEND_SCR: u32 = 51;
}

pub mod MMC_RSP {
    pub const PRESENT: u32 = 1 << 0;
    pub const _136: u32 = 1 << 1;
    pub const CRC: u32 = 1 << 2;
    pub const BUSY: u32 = 1 << 3;
    pub const OPCODE: u32 = 1 << 4;
}
#[repr(u32)]
enum MMC_RSP_R {
    NONE = (0 << 24),
    _1 = (1 << 24) | (MMC_RSP::PRESENT | MMC_RSP::CRC | MMC_RSP::OPCODE),
    _1B = (1 << 24) | (MMC_RSP::PRESENT | MMC_RSP::CRC | MMC_RSP::OPCODE | MMC_RSP::BUSY),
    _2 = (2 << 24) | (MMC_RSP::PRESENT | MMC_RSP::_136 | MMC_RSP::CRC),
    _3 = (3 << 24) | (MMC_RSP::PRESENT),
    _4 = (4 << 24) | (MMC_RSP::PRESENT),
    _5 = (5 << 24) | (MMC_RSP::PRESENT | MMC_RSP::CRC | MMC_RSP::OPCODE),
    _6 = (6 << 24) | (MMC_RSP::PRESENT | MMC_RSP::CRC | MMC_RSP::OPCODE),
    _7 = (7 << 24) | (MMC_RSP::PRESENT | MMC_RSP::CRC | MMC_RSP::OPCODE),
}
#[repr(u32)]
enum MMC_Status {
    IDLE = 0,
    READY = 1,
    IDENT = 2,
    STBY = 3,
    TRAN = 4,
    DATA = 5,
    RCV = 6,
    PRG = 7,
    DIS = 8,
    BTST = 9,
    SLP = 10,
}
#[repr(u32)]
enum OCR {
    BUSY = 0x80000000,
    HCS = 0x40000000,
    VOLTAGE_MASK = 0x00ffff80,
    ACCESS_MODE = 0x60000000,
}
#[repr(u32)]
pub enum MMC_Data {
    READ = (1 << 0),
    WRITE = (1 << 1),
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum MMC_Vdd {
    V27_36 = (1 << 0),
    V165_195 = (1 << 1),
}

#[repr(u32)]
enum SD_MMC_VERSION {
    _SD = 0x20000,
    SDV_3 = (0x20000 | 0x300),
    SDV_2 = (0x20000 | 0x200),
    SDV_1_0 = (0x20000 | 0x100),
    SDV_1_10 = (0x20000 | 0x10a),
    _MMC = 0x10000,
    // V_UNKNOWN 	= (0x10000 |0x0),
    _1_2 = (0x10000 | 0x102),
    _1_4 = (0x10000 | 0x104),
    _2_2 = (0x10000 | 0x202),
    _3 = (0x10000 | 0x300),
    _4 = (0x10000 | 0x400),
    _4_1 = (0x10000 | 0x401),
    _4_2 = (0x10000 | 0x402),
    _4_3 = (0x10000 | 0x403),
    _4_41 = (0x10000 | 0x429),
    _4_5 = (0x10000 | 0x405),
    _5_0 = (0x10000 | 0x500),
    _5_1 = (0x10000 | 0x501),
}

const tran_speed_unit: [u32; 4] = [10000, 100000, 1000000, 10000000];
const tran_speed_time: [u8; 16] = [
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
    pub fn init(&mut self) -> bool {
        self.online = false;

        if sdcard_detect(&mut self.hci, &mut self.card) {
            self.online = true;
            print!("SD/MMC card found\r\n");
            return true;
        }
        print!("SD/MMC card not found\r\n");
        false
    }

    pub fn blk_read(&mut self, mut buf: *mut u8, mut blkno: u64, blkcnt: u64) -> u64 {
        let mut blks = blkcnt;
        let card = self.card;

        while blks > 0 {
            let cnt = if blks > 127 { 127 } else { blks };
            if mmc_read_blocks(&mut self.hci, &card, buf, blkno, cnt) != cnt {
                return 0;
            }
            blks -= cnt;
            blkno += cnt;
            let off = buf as *const _ as u32 + (cnt as u32 * card.read_bl_len);
            buf = off as *const u32 as *mut u8;
        }
        blkcnt
    }
}

fn mmc_read_blocks(hci: &mut Sdhci, card: &Sdcard, buf: *mut u8, start: u64, blkcnt: u64) -> u64 {
    let mut cmd = SdhciCmd::default();
    let dat = SdhciData {
        buf,
        flag: MMC_Data::READ as u32,
        blksz: card.read_bl_len,
        blkcnt: blkcnt as u32,
    };
    let mut status = 0i32;

    if blkcnt > 1 {
        cmd.idx = MMC::READ_MULTIPLE_BLOCK as u32;
    } else {
        cmd.idx = MMC::READ_SINGLE_BLOCK as u32;
    }
    if card.high_capacity != 0 {
        cmd.arg = start as u32;
    } else {
        cmd.arg = start as u32 * card.read_bl_len;
    }
    cmd.resptype = MMC_RSP_R::_1 as u32;

    if !sdhci_transfer(hci, &mut cmd, Some(&dat)) {
        return 0;
    }

    if !hci.isspi {
        loop {
            status = mmc_status(hci, card);
            if status < 0 {
                return 0;
            }

            if !((status != MMC_Status::TRAN as i32) && (status != MMC_Status::DATA as i32)) {
                break;
            }
        }
    }

    if blkcnt > 1 {
        cmd.idx = MMC::STOP_TRANSMISSION as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_1B as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return 0;
        }
    }
    blkcnt
}

fn go_idle_state(hci: &mut Sdhci) -> bool {
    let mut cmd = SdhciCmd::default();
    cmd.idx = MMC::GO_IDLE_STATE as u32;
    cmd.arg = 0;
    cmd.resptype = if hci.isspi {
        MMC_RSP_R::_1 as u32
    } else {
        MMC_RSP_R::NONE as u32
    };

    if sdhci_transfer(hci, &mut cmd, None) {
        return true;
    }

    sdhci_transfer(hci, &mut cmd, None)
}

fn mmc_status(hci: &mut Sdhci, card: &Sdcard) -> i32 {
    let mut cmd = SdhciCmd::default();
    let mut retries = 100i32;

    cmd.idx = MMC::SEND_STATUS as u32;
    cmd.resptype = MMC_RSP_R::_1 as u32;
    cmd.arg = card.rca << 16;
    loop {
        if !sdhci_transfer(hci, &mut cmd, None) {
            continue;
        }
        if (cmd.response[0] & (1 << 8)) != 0 {
            break;
        }
        udelay(1);
        retries -= 1;
        if retries == 0 {
            break;
        }
    }
    if retries > 0 {
        return ((cmd.response[0] >> 9) & 0xf).try_into().unwrap();
    }
    -1
}

fn sd_send_if_cond(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd::default();

    cmd.idx = SD_Cmd::SEND_IF_COND;
    if (hci.voltage & MMC_Vdd::V27_36 as u32) != 0 {
        cmd.arg = 0x1 << 8;
    } else if (hci.voltage & MMC_Vdd::V165_195 as u32) != 0 {
        cmd.arg = 0x2 << 8;
    } else {
        cmd.arg = 0x0 << 8;
    }
    cmd.arg |= 0xaa;
    cmd.resptype = MMC_RSP_R::_7 as u32;
    if !sdhci_transfer(hci, &mut cmd, None) {
        return false;
    }

    if (cmd.response[0] & 0xff) != 0xaa {
        return false;
    }
    card.version = SD_MMC_VERSION::SDV_2 as u32;
    true
}

#[allow(clippy::never_loop, clippy::while_immutable_condition)]
fn mmc_send_op_cond(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd::default();

    let mut retries = 100i32;

    if !go_idle_state(hci) {
        return false;
    }

    cmd.idx = MMC::SEND_OP_COND as u32;
    cmd.arg = 0;
    cmd.resptype = MMC_RSP_R::_3 as u32;
    if !sdhci_transfer(hci, &mut cmd, None) {
        return false;
    }

    loop {
        cmd.idx = MMC::SEND_OP_COND as u32;
        cmd.arg = if hci.isspi {
            0
        } else {
            (card.ocr & OCR::VOLTAGE_MASK as u32) | (card.ocr & OCR::ACCESS_MODE as u32)
        };
        cmd.arg |= OCR::HCS as u32;
        cmd.resptype = MMC_RSP_R::_3 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        retries -= 1;
        while (cmd.response[0] & OCR::BUSY as u32) == 0 && retries == 0 {
            break;
        }
    }

    if retries <= 0 {
        return false;
    }

    if hci.isspi {
        cmd.idx = MMC::SPI_READ_OCR as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_3 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
    }
    card.version = SD_MMC_VERSION::_MMC as u32;
    card.ocr = cmd.response[0];
    card.high_capacity = ((card.ocr & OCR::HCS as u32) == OCR::HCS as u32) as u32;
    card.rca = 0;
    true
}

fn sd_send_op_cond(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd::default();
    let mut retries = 100i32;

    loop {
        cmd.idx = MMC::APP_CMD as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_1 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            retries -= 1;
            if retries == 0 {
                break;
            }
            continue;
        }

        cmd.idx = SD_Cmd::APP_SEND_OP_COND;
        if hci.isspi {
            cmd.arg = 0;
            if card.version == SD_MMC_VERSION::SDV_2 as u32 {
                cmd.arg |= OCR::HCS as u32;
            }
            cmd.resptype = MMC_RSP_R::_1 as u32;
            if sdhci_transfer(hci, &mut cmd, None) {
                break;
            }
        } else {
            if (hci.voltage & MMC_Vdd::V27_36 as u32) != 0 {
                cmd.arg = 0x00ff8000;
            } else if (hci.voltage & MMC_Vdd::V165_195 as u32) != 0 {
                cmd.arg = 0x00000080;
            } else {
                cmd.arg = 0;
            }
            if card.version == SD_MMC_VERSION::SDV_2 as u32 {
                cmd.arg |= OCR::HCS as u32;
            }
            cmd.resptype = MMC_RSP_R::_3 as u32;
            if !sdhci_transfer(hci, &mut cmd, None) || (cmd.response[0] & OCR::BUSY as u32) != 0 {
                break;
            }
        }
        retries -= 1;
        if retries == 0 {
            break;
        }
    }

    if retries <= 0 {
        return false;
    }

    if card.version != SD_MMC_VERSION::SDV_2 as u32 {
        card.version = SD_MMC_VERSION::SDV_1_0 as u32;
    }
    if hci.isspi {
        cmd.idx = MMC::SPI_READ_OCR as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_3 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
    }
    card.ocr = cmd.response[0];
    card.high_capacity = ((card.ocr & OCR::HCS as u32) == OCR::HCS as u32) as u32;
    card.rca = 0;

    true
}

fn sdcard_detect(hci: &mut Sdhci, card: &mut Sdcard) -> bool {
    let mut cmd = SdhciCmd::default();
    let mut csize = 0u64;
    let mut cmult = 0u64;
    let mut unit = 0u32;
    let mut time = 0u32;
    let mut width = 0i32;
    let mut status = 0i32;

    sdhci_reset(hci);
    if !sdhci_set_clock(hci, MMC_CLK_400K) || !sdhci_set_width(hci, MMC_BUS_WIDTH_1) {
        print!("SMHC: set clock/width failed\r\n");
        return false;
    }

    if !go_idle_state(hci) {
        print!("get state\r\n");
        return false;
    }
    udelay(2000);

    sd_send_if_cond(hci, card);

    if !sd_send_op_cond(hci, card) {
        if !mmc_send_op_cond(hci, card) {
            return false;
        }
    }

    if hci.isspi {
        cmd.idx = MMC::SEND_CID as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_1 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.cid[0] = cmd.response[0];
        card.cid[1] = cmd.response[1];
        card.cid[2] = cmd.response[2];
        card.cid[3] = cmd.response[3];

        cmd.idx = MMC::SEND_CSD as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_1 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.csd[0] = cmd.response[0];
        card.csd[1] = cmd.response[1];
        card.csd[2] = cmd.response[2];
        card.csd[3] = cmd.response[3];
    } else {
        cmd.idx = MMC::ALL_SEND_CID as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_2 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.cid[0] = cmd.response[0];
        card.cid[1] = cmd.response[1];
        card.cid[2] = cmd.response[2];
        card.cid[3] = cmd.response[3];

        cmd.idx = SD_Cmd::SEND_RELATIVE_ADDR;
        cmd.arg = card.rca << 16;
        cmd.resptype = MMC_RSP_R::_6 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        if (card.version & SD_MMC_VERSION::_SD as u32) != 0 {
            card.rca = (cmd.response[0] >> 16) & 0xffff;
        }

        cmd.idx = MMC::SEND_CSD as u32;
        cmd.arg = card.rca << 16;
        cmd.resptype = MMC_RSP_R::_2 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        card.csd[0] = cmd.response[0];
        card.csd[1] = cmd.response[1];
        card.csd[2] = cmd.response[2];
        card.csd[3] = cmd.response[3];

        cmd.idx = MMC::SELECT_CARD as u32;
        cmd.arg = card.rca << 16;
        cmd.resptype = MMC_RSP_R::_1 as u32;
        if !sdhci_transfer(hci, &mut cmd, None) {
            return false;
        }
        loop {
            status = mmc_status(hci, card);
            if status < 0 {
                return false;
            }
            if status == (MMC_Status::TRAN as u32).try_into().unwrap() {
                break;
            };
        }
    }

    if card.version == SD_MMC_VERSION::_MMC as u32 {
        card.version = match (card.csd[0] >> 26) & 0xf {
            0 => SD_MMC_VERSION::_1_2 as u32,
            1 => SD_MMC_VERSION::_1_4 as u32,
            2 => SD_MMC_VERSION::_2_2 as u32,
            3 => SD_MMC_VERSION::_3 as u32,
            4 => SD_MMC_VERSION::_4 as u32,
            _ => SD_MMC_VERSION::_1_2 as u32,
        };
    }

    unit = tran_speed_unit[(card.csd[0] & 0x7) as usize];
    time = tran_speed_time[((card.csd[0] >> 3) & 0xf) as usize] as u32;
    card.tran_speed = time * unit;
    card.dsr_imp = UNSTUFF_BITS(card.csd, 76, 1) as u32;

    card.read_bl_len = 1 << UNSTUFF_BITS(card.csd, 80, 4);
    if (card.version & SD_MMC_VERSION::_SD as u32) != 0 {
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

    if ((card.version & SD_MMC_VERSION::_MMC as u32) != 0
        && (card.version >= SD_MMC_VERSION::_4 as u32))
    {
        cmd.idx = MMC::SEND_EXT_CSD as u32;
        cmd.arg = 0;
        cmd.resptype = MMC_RSP_R::_1 as u32;
        let dat = SdhciData {
            buf: card.extcsd.as_mut_ptr(),
            flag: MMC_Data::READ as u32,
            blksz: 512,
            blkcnt: 1,
        };
        if !sdhci_transfer(hci, &mut cmd, Some(&dat)) {
            return false;
        }
        if !hci.isspi {
            loop {
                status = mmc_status(hci, card);
                if status < 0 {
                    return false;
                }
                if status == (MMC_Status::TRAN as u32).try_into().unwrap() {
                    break;
                }
            }
        }
        card.version = match card.extcsd[192] {
            1 => SD_MMC_VERSION::_4_1 as u32,
            2 => SD_MMC_VERSION::_4_2 as u32,
            3 => SD_MMC_VERSION::_4_3 as u32,
            5 => SD_MMC_VERSION::_4_41 as u32,
            6 => SD_MMC_VERSION::_4_5 as u32,
            7 => SD_MMC_VERSION::_5_0 as u32,
            8 => SD_MMC_VERSION::_5_1 as u32,
            _ => 0,
        }
    }

    if card.high_capacity != 0 {
        if (card.version & SD_MMC_VERSION::_SD as u32) != 0 {
            csize = UNSTUFF_BITS(card.csd, 48, 22);
            card.capacity = (1 + csize) << 10;
        } else {
            card.capacity = (card.extcsd[212] as u64) << 0
                | (card.extcsd[212 + 1] as u64) << 8
                | (card.extcsd[212 + 2] as u64) << 16
                | (card.extcsd[212 + 3] as u64) << 24;
        }
    } else {
        cmult = UNSTUFF_BITS(card.csd, 47, 3);
        csize = UNSTUFF_BITS(card.csd, 62, 12);
        card.capacity = (csize + 1) << (cmult + 2);
    }
    card.capacity *= 1 << UNSTUFF_BITS(card.csd, 80, 4);

    if hci.isspi {
        sdhci_set_clock(hci, min(card.tran_speed, hci.clock_wanted));
        sdhci_set_width(hci, MMC_BUS_WIDTH_1);
    } else {
        if (card.version & SD_MMC_VERSION::_SD as u32) != 0 {
            if hci.width == MMC_BUS_WIDTH_4 {
                width = 2;
            } else {
                width = 0;
            }

            cmd.idx = MMC::APP_CMD as u32;
            cmd.arg = card.rca << 16;
            cmd.resptype = MMC_RSP_R::_5 as u32;
            if !sdhci_transfer(hci, &mut cmd, None) {
                return false;
            }

            cmd.idx = SD_Cmd::SWITCH_FUNC as u32;
            cmd.arg = width as u32;
            cmd.resptype = MMC_RSP_R::_1 as u32;
            if !sdhci_transfer(hci, &mut cmd, None) {
                return false;
            }
            if !sdhci_set_clock(hci, hci.clock_wanted)
                || !sdhci_set_width(
                    hci,
                    if hci.width == MMC_BUS_WIDTH_4 {
                        MMC_BUS_WIDTH_4
                    } else {
                        MMC_BUS_WIDTH_1
                    },
                )
            {
                print!("SMHC: set clock/width failed\r\n");
                return false;
            }
        } else if (card.version & SD_MMC_VERSION::_MMC as u32) != 0 {
            if hci.width == MMC_BUS_WIDTH_4 {
                width = 1;
            } else {
                width = 0;
            }

            cmd.idx = MMC::APP_CMD as u32;
            cmd.arg = card.rca << 16;
            cmd.resptype = MMC_RSP_R::_5 as u32;
            if !sdhci_transfer(hci, &mut cmd, None) {
                return false;
            }

            cmd.idx = SD_Cmd::SWITCH_FUNC as u32;
            cmd.arg = width as u32;
            cmd.resptype = MMC_RSP_R::_1 as u32;
            if !sdhci_transfer(hci, &mut cmd, None) {
                return false;
            }

            if !sdhci_set_clock(hci, hci.clock_wanted) || !sdhci_set_width(hci, hci.width) {
                print!("SMHC: set clock/width failed\r\n");
                return false;
            }
        }
    }

    cmd.idx = MMC::SET_BLOCKLEN as u32;
    cmd.arg = card.read_bl_len;
    cmd.resptype = MMC_RSP_R::_1 as u32;
    if !sdhci_transfer(hci, &mut cmd, None) {
        return false;
    }

    true
}

fn UNSTUFF_BITS(resp: [u32; 4], start: u32, size: u32) -> u64 {
    let __size = size;
    let __mask = if (__size < 32) { 1 << __size } else { 0 } - 1;
    let __off = (3 - ((start) / 32)) as usize;
    let __shft = (start) & 31;
    let mut __res;

    __res = resp[__off] >> __shft;
    if (__size + __shft > 32) {
        __res |= resp[__off - 1] << ((32 - __shft) % 32);
    }
    (__res & __mask) as u64
}
