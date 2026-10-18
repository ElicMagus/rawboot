// SPDX-License-Identifier: Unlicense

#![allow(warnings)]
use core::{
    default,
    slice::{self, from_raw_parts_mut},
};

use rawboot_rt::cpu::{get_sys_ticks, udelay};

use crate::{
    clk,
    gpio::{self},
    io::{read32, write32},
    print,
    sdcard::{MMC, MMC_Bus_Width, MMC_Data, MMC_RSP, MMC_Vdd},
};

mod SMHC {
    pub const CTRL: u32 = 0x00;
    pub const CLKDIV: u32 = 0x04;
    pub const TMOUT: u32 = 0x08;
    pub const CTYPE: u32 = 0x0c;
    pub const BLKSIZ: u32 = 0x10;
    pub const BYTCNT: u32 = 0x14;
    pub const CMDR: u32 = 0x18;
    pub const CMDARG: u32 = 0x1c;
    pub const RESP0: u32 = 0x20;
    pub const RESP1: u32 = 0x24;
    pub const RESP2: u32 = 0x28;
    pub const RESP3: u32 = 0x2c;
    pub const INTMASK: u32 = 0x30;
    pub const MISR: u32 = 0x34;
    pub const RISR: u32 = 0x38; //Raw Interrupt Status Register
    pub const STATUS: u32 = 0x3c;
    pub const FWLR: u32 = 0x40;
    pub const FUNS: u32 = 0x44;
    pub const TCBCNT: u32 = 0x48;
    pub const TBBCNT: u32 = 0x4C;
    pub const DBGC: u32 = 0x50;
    pub const CSDC: u32 = 0x54; //CRC Status Detect Control Registers 
    pub const A12A: u32 = 0x58; //Auto Command 12 Argument Register 
    pub const NTSR: u32 = 0x5c;
    pub const SDBG: u32 = 0x60;
    pub const HWRST: u32 = 0x78;
    pub const DMAC: u32 = 0x80;
    pub const DLBA: u32 = 0x84;
    pub const IDST: u32 = 0x88;
    pub const IDIE: u32 = 0x8c;
    pub const THLDC: u32 = 0x100;
    pub const SFC: u32 = 0x104; //Sample FIFO Control Register 
    pub const A23A: u32 = 0x108; //Auto Command 23 Argument Register
    pub const DSBD: u32 = 0x10c; //eMMC4.5 DDR Start Bit Detection Control Register
    pub const RES_CRC: u32 = 0x110;
    pub const DATA7_CRC: u32 = 0x114;
    pub const DATA6_CRC: u32 = 0x118;
    pub const DATA5_CRC: u32 = 0x11c;
    pub const DATA4_CRC: u32 = 0x120;
    pub const DATA3_CRC: u32 = 0x124;
    pub const DATA2_CRC: u32 = 0x128;
    pub const DATA1_CRC: u32 = 0x12c;
    pub const DATA0_CRC: u32 = 0x130;
    pub const CRC_STA: u32 = 0x134;
    pub const EXT_CMD: u32 = 0x0138; // Extended Command Register
    pub const EXT_RESP: u32 = 0x013C; // Extended Response Register 
    pub const DRV_DL: u32 = 0x0140; // Drive Delay Control Register 
    pub const SMAP_DL: u32 = 0x0144; // Sample Delay Control Register
    pub const DS_DL: u32 = 0x0148; // Data Strobe Delay Control Register   
    pub const HS400_DL: u32 = 0x014C; // HS400 Delay Control Register 
    pub const FIFO: u32 = 0x0200; // Read/Write FIFO
}

mod SDXC {
    pub const SOFT_RESET: u32 = (1 << 0);
    pub const FIFO_RESET: u32 = (1 << 1);
    pub const DMA_RESET: u32 = (1 << 2);
    pub const INTERRUPT_ENABLE_BIT: u32 = (1 << 4);
    pub const DMA_ENABLE_BIT: u32 = (1 << 5);
    pub const DEBOUNCE_ENABLE_BIT: u32 = (1 << 8);
    pub const POSEDGE_LATCH_DATA: u32 = (1 << 9);
    pub const DDR_MODE: u32 = (1 << 10);
    pub const MEMORY_ACCESS_DONE: u32 = (1 << 29);
    pub const ACCESS_DONE_DIRECT: u32 = (1 << 30);
    pub const ACCESS_BY_AHB: u32 = (1 << 31);
    pub const ACCESS_BY_DMA: u32 = (0 << 31);
    pub const HARDWARE_RESET: u32 = (SOFT_RESET | FIFO_RESET | DMA_RESET);
    pub const WIDTH1: u32 = 0;
    pub const WIDTH4: u32 = 1;
    pub const WIDTH8: u32 = 2;
    //commands
    pub const RESP_EXPIRE: u32 = (1 << 6);
    pub const LONG_RESPONSE: u32 = (1 << 7);
    pub const CHECK_RESPONSE_CRC: u32 = (1 << 8);
    pub const DATA_EXPIRE: u32 = (1 << 9);
    pub const WRITE: u32 = (1 << 10);
    pub const SEQUENCE_MODE: u32 = (1 << 11);
    pub const SEND_AUTO_STOP: u32 = (1 << 12);
    pub const WAIT_PRE_OVER: u32 = (1 << 13);
    pub const STOP_ABORT_CMD: u32 = (1 << 14);
    pub const SEND_INIT_SEQUENCE: u32 = (1 << 15);
    pub const UPCLK_ONLY: u32 = (1 << 21);
    pub const READ_CEATA_DEV: u32 = (1 << 22);
    pub const CCS_EXPIRE: u32 = (1 << 23);
    pub const ENABLE_BIT_BOOT: u32 = (1 << 24);
    pub const ALT_BOOT_OPTIONS: u32 = (1 << 25);
    pub const BOOT_ACK_EXPIRE: u32 = (1 << 26);
    pub const BOOT_ABORT: u32 = (1 << 27);
    pub const VOLTAGE_SWITCH: u32 = (1 << 28);
    pub const USE_HOLD_REGISTER: u32 = (1 << 29);
    pub const START: u32 = (1 << 31);
    //status
    pub const RXWL_FLAG: u32 = (1 << 0);
    pub const TXWL_FLAG: u32 = (1 << 1);
    pub const FIFO_EMPTY: u32 = (1 << 2);
    pub const FIFO_FULL: u32 = (1 << 3);
    pub const CARD_PRESENT: u32 = (1 << 8);
    pub const CARD_DATA_BUSY: u32 = 1 << 9;
    pub const DATA_FSM_BUSY: u32 = 1 << 10;
    pub const DMA_REQUEST: u32 = 1 << 31;
    pub const FIFO_SIZE: u32 = (16);
    //interrupts bit
    pub const RESP_ERROR: u32 = (1 << 1);
    pub const COMMAND_DONE: u32 = (1 << 2);
    pub const DATA_OVER: u32 = (1 << 3);
    pub const TX_DATA_REQUEST: u32 = (1 << 4);
    pub const RX_DATA_REQUEST: u32 = (1 << 5);
    pub const RESP_CRC_ERROR: u32 = (1 << 6);
    pub const DATA_CRC_ERROR: u32 = (1 << 7);
    pub const RESP_TIMEOUT: u32 = (1 << 8);
    pub const DATA_TIMEOUT: u32 = (1 << 9);
    pub const VOLTAGE_CHANGE_DONE: u32 = 1 << 10;
    pub const FIFO_RUN_ERROR: u32 = 1 << 11;
    pub const HARD_WARE_LOCKED: u32 = 1 << 12;
    pub const START_BIT_ERROR: u32 = 1 << 13;
    pub const AUTO_COMMAND_DONE: u32 = 1 << 14;
    pub const END_BIT_ERROR: u32 = 1 << 15;
    pub const SDIO_INTERRUPT: u32 = 1 << 16;
    pub const CARD_INSERT: u32 = 1 << 30;
    pub const CARD_REMOVE: u32 = 1 << 31;
    pub const INTERRUPT_ERROR_BIT: u32 = (RESP_ERROR
        | RESP_CRC_ERROR
        | DATA_CRC_ERROR
        | RESP_TIMEOUT
        | DATA_TIMEOUT
        | FIFO_RUN_ERROR
        | HARD_WARE_LOCKED
        | START_BIT_ERROR
        | END_BIT_ERROR);
    pub const INTERRUPT_DONE_BIT: u32 =
        (AUTO_COMMAND_DONE | DATA_OVER | COMMAND_DONE | VOLTAGE_CHANGE_DONE);
}

#[derive(Default, Clone, Copy)]
pub struct sdhci_cmd_t {
    pub cmdidx: u32,
    pub cmdarg: u32,
    pub resptype: u32,
    pub response: [u32; 4],
}
#[derive(Clone, Copy)]
pub struct sdhci_data_t {
    pub buf: *mut u8, //указатель на буфер
    pub flag: u32,
    pub blksz: u32,
    pub blkcnt: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct sdhci_t {
    pub name: &'static [u8], //указатель
    pub addr: u32,
    pub pclk: u32,
    pub reset: u32,
    pub voltage: MMC_Vdd,
    pub width: MMC_Bus_Width,
    pub clock: u32,
    pub removable: bool,
    pub isspi: bool,

    gpio_d0: gpio::Gpio,
    gpio_d1: gpio::Gpio,
    gpio_d2: gpio::Gpio,
    gpio_d3: gpio::Gpio,
    gpio_cmd: gpio::Gpio,
    gpio_clk: gpio::Gpio,

    sdcard: *mut u32, //адрес по которому хранится структура для SD карты
}

impl Default for sdhci_t {
    fn default() -> sdhci_t {
        sdhci_t {
            name: b"sdhci0",
            addr: 0x04020000,
            pclk: 0,
            reset: 400,
            voltage: MMC_Vdd::V27_36,
            width: MMC_Bus_Width::W4,
            clock: 25 * 1000 * 1000,
            removable: false,
            isspi: false,
            gpio_d0: gpio::Gpio {
                port: gpio::Port::F,
                pin: 1,
                func: gpio::FUNC::MUX2,
                pull_mode: gpio::Pull::UP,
            },
            gpio_d1: gpio::Gpio {
                port: gpio::Port::F,
                pin: 0,
                func: gpio::FUNC::MUX2,
                pull_mode: gpio::Pull::UP,
            },
            gpio_d2: gpio::Gpio {
                port: gpio::Port::F,
                pin: 5,
                func: gpio::FUNC::MUX2,
                pull_mode: gpio::Pull::UP,
            },
            gpio_d3: gpio::Gpio {
                port: gpio::Port::F,
                pin: 4,
                func: gpio::FUNC::MUX2,
                pull_mode: gpio::Pull::UP,
            },
            gpio_cmd: gpio::Gpio {
                port: gpio::Port::F,
                pin: 3,
                func: gpio::FUNC::MUX2,
                pull_mode: gpio::Pull::UP,
            },
            gpio_clk: gpio::Gpio {
                port: gpio::Port::F,
                pin: 2,
                func: gpio::FUNC::MUX2,
                pull_mode: gpio::Pull::UP,
            },
            sdcard: 0 as *mut u32,
        }
    }
}

impl sdhci_t {
    pub fn reset(mut self) -> bool {
        write32(self.addr + SMHC::CTRL, SDXC::HARDWARE_RESET);
        return true;
    }
    pub fn set_voltage(mut self, voltage: u32) -> bool {
        false
    }
    pub fn set_width(mut self, width: MMC_Bus_Width) -> bool {
        match (width) {
            MMC_Bus_Width::W1 => write32(self.addr + SMHC::CTYPE, SDXC::WIDTH1),

            MMC_Bus_Width::W4 => write32(self.addr + SMHC::CTYPE, SDXC::WIDTH4),

            MMC_Bus_Width::W8 => write32(self.addr + SMHC::CTYPE, SDXC::WIDTH8),

            _ => return false,
        }
        return true;
    }
    pub fn set_clock(mut self, clock: u32) -> bool {
        let ratio: u32 = (self.pclk + 2 * clock - 1) / (2 * clock);

        if ((ratio & 0xff) != ratio) {
            return false;
        }

        write32(
            self.addr + SMHC::CLKDIV,
            read32(self.addr + SMHC::CLKDIV) & !(1 << 16),
        );
        write32(self.addr + SMHC::CLKDIV, ratio);
        if (!self.update_clk()) {
            return false;
        }

        write32(
            self.addr + SMHC::CLKDIV,
            read32(self.addr + SMHC::CLKDIV) | (3 << 16),
        );
        if (!self.update_clk()) {
            return false;
        }

        return true;
    }
    pub fn update_clk(&mut self) -> bool {
        let mut cmd: u32 = (1u32 << 31) | (1 << 21) | (1 << 13);

        write32(self.addr + SMHC::CMDR, cmd);
        let timeout = get_sys_ticks();
        loop {
            if (get_sys_ticks() - timeout > 10) {
                return false;
            }
            if (read32(self.addr + SMHC::CMDR) & 0x80000000) != 0 {
                break;
            }
        }
        write32(self.addr + SMHC::RISR, read32(self.addr + SMHC::RISR));
        return true;
    }

    pub fn transfer(mut self, cmd: sdhci_cmd_t, dat: sdhci_data_t) -> bool {
        if dat.buf == (0 as *mut u8) {
            return self.transfer_command(cmd, dat);
        }
        return self.transfer_data(cmd, dat);
    }

    pub fn transfer_command(mut self, mut cmd: sdhci_cmd_t, dat: sdhci_data_t) -> bool {
        let mut cmdval: u32 = SDXC::START;
        let mut status = 0u32;
        let mut timeout: u32 = 0;

        if (cmd.cmdidx == MMC::STOP_TRANSMISSION as u32) {
            timeout = get_sys_ticks();
            loop {
                status = read32(self.addr + SMHC::STATUS);
                if (get_sys_ticks() - timeout > 10) {
                    write32(self.addr + SMHC::CTRL, SDXC::HARDWARE_RESET);
                    write32(self.addr + SMHC::RISR, 0xffffffff);
                    return false;
                }
                if (status & SDXC::CARD_DATA_BUSY) == 0 {
                    break;
                }
            }
            return true;
        }

        if (cmd.cmdidx == MMC::GO_IDLE_STATE as u32) {
            cmdval |= SDXC::SEND_INIT_SEQUENCE;
        }
        if (cmd.resptype & MMC_RSP::PRESENT != 0) {
            cmdval |= SDXC::RESP_EXPIRE;
            if (cmd.resptype & MMC_RSP::_136) != 0 {
                cmdval |= SDXC::LONG_RESPONSE;
            }
            if (cmd.resptype & MMC_RSP::CRC) != 0 {
                cmdval |= SDXC::CHECK_RESPONSE_CRC;
            }
        }

        if (dat.buf != 0 as *mut u8) {
            cmdval |= SDXC::DATA_EXPIRE | SDXC::WAIT_PRE_OVER;
        }

        if (dat.buf != 0 as *mut u8 && (dat.flag & MMC_Data::WRITE as u32) != 0) {
            cmdval |= SDXC::WRITE;
        }

        if (cmd.cmdidx == MMC::WRITE_MULTIPLE_BLOCK as u32
            || cmd.cmdidx == MMC::READ_MULTIPLE_BLOCK as u32)
        {
            cmdval |= SDXC::SEND_AUTO_STOP;
        }

        write32(self.addr + SMHC::CMDARG, cmd.cmdarg);

        if (dat.buf != 0 as *mut u8) {
            write32(
                self.addr + SMHC::CTRL,
                read32(self.addr + SMHC::CTRL) | 0x80000000,
            );
        }

        write32(self.addr + SMHC::CMDR, cmdval | cmd.cmdidx); //послать команду

        timeout = get_sys_ticks();

        loop {
            status = read32(self.addr + SMHC::RISR);
            if ((get_sys_ticks() - timeout > 10) || (status & SDXC::INTERRUPT_ERROR_BIT) != 0) {
                write32(self.addr + SMHC::CTRL, SDXC::HARDWARE_RESET);
                write32(self.addr + SMHC::RISR, 0xffffffff);
                return false;
            }
            if (status & SDXC::COMMAND_DONE) != 0 {
                break;
            }
        }

        if (cmd.resptype & MMC_RSP::BUSY as u32) != 0 {
            timeout = get_sys_ticks();
            loop {
                status = read32(self.addr + SMHC::STATUS);
                if (get_sys_ticks() - timeout > 10) {
                    write32(self.addr + SMHC::CTRL, SDXC::HARDWARE_RESET);
                    write32(self.addr + SMHC::RISR, 0xffffffff);
                    return false;
                }
                if (status & (1 << 9)) != 0 {
                    break;
                }
            }
        }

        if (cmd.resptype & MMC_RSP::_136) != 0 {
            cmd.response[0] = read32(self.addr + SMHC::RESP3);
            cmd.response[1] = read32(self.addr + SMHC::RESP2);
            cmd.response[2] = read32(self.addr + SMHC::RESP1);
            cmd.response[3] = read32(self.addr + SMHC::RESP0);
        } else {
            cmd.response[0] = read32(self.addr + SMHC::RESP0);
        }

        write32(self.addr + SMHC::RISR, 0xffffffff);

        return true;
    }

    pub fn transfer_data(mut self, cmd: sdhci_cmd_t, dat: sdhci_data_t) -> bool {
        let mut dlen: u32 = (dat.blkcnt * dat.blksz);
        let mut ret: bool = false;

        write32(self.addr + SMHC::BLKSIZ, dat.blksz);
        write32(self.addr + SMHC::BYTCNT, dlen);
        if (dat.flag & MMC_Data::READ as u32) != 0 {
            if (!self.transfer_command(cmd, dat)) {
                return false;
            }
            ret = self.read_bytes(dat.buf as *const _ as *mut u32, dat.blkcnt, dat.blksz);
        } else if (dat.flag & MMC_Data::WRITE as u32) != 0 {
            if (!self.transfer_command(cmd, dat)) {
                return false;
            }
            ret = self.write_bytes(dat.buf as *const _ as *mut u32, dat.blkcnt, dat.blksz);
        }
        return ret;
    }

    pub fn write_bytes(mut self, buf: *mut u32, blkcount: u32, blksize: u32) -> bool {
        let mut count: u64 = (blkcount * blksize).into();
        let mut tmp =
            unsafe { slice::from_raw_parts_mut(buf as *const _ as *mut u32, count as usize) };
        let mut status = 0u32;
        let mut err = 0u32;
        let mut done = 0u32;

        status = read32(self.addr + SMHC::STATUS);
        err = read32(self.addr + SMHC::RISR) & SDXC::INTERRUPT_ERROR_BIT;
        let mut i = 0;
        while (err == 0 && count != 0) {
            if ((status & SDXC::FIFO_FULL) == 0) {
                unsafe {
                    write32(self.addr + SMHC::FIFO, tmp[i] as u32);
                }
                i += 1;
                count -= 4;
            }
            status = read32(self.addr + SMHC::STATUS);
            err = read32(self.addr + SMHC::RISR) & SDXC::INTERRUPT_ERROR_BIT;
        }

        loop {
            status = read32(self.addr + SMHC::RISR);
            err = status & SDXC::INTERRUPT_ERROR_BIT;
            if (blkcount > 1) {
                done = status & SDXC::AUTO_COMMAND_DONE;
            } else {
                done = status & SDXC::DATA_OVER;
            }
            if (done != 0 && err != 0) {
                break;
            }
        }

        if (err & SDXC::INTERRUPT_ERROR_BIT) != 0 {
            return false;
        }
        write32(
            self.addr + SMHC::CTRL,
            read32(self.addr + SMHC::CTRL) | SDXC::FIFO_RESET,
        );
        write32(self.addr + SMHC::RISR, 0xffffffff);

        if (count != 0) {
            return false;
        }
        return true;
    }

    #[allow(clippy::erasing_op)]
    pub fn read_bytes(mut self, buf: *mut u32, blkcount: u32, blksize: u32) -> bool {
        let mut count: u64 = (blkcount * blksize).into();
        let mut tmp =
            unsafe { slice::from_raw_parts_mut(buf as *const _ as *mut u32, count as usize) };
        let mut status: u32 = 0;
        let mut err: u32 = 0;
        let mut done: u32 = 0;

        status = read32(self.addr + SMHC::STATUS);
        err = read32(self.addr + SMHC::RISR) & SDXC::INTERRUPT_ERROR_BIT;

        let mut i = 0;
        while ((err == 0) && (count >= 4)) {
            if ((status & SDXC::FIFO_EMPTY) == 0) {
                tmp[i] = read32(self.addr + SMHC::FIFO);
                i += 1;
                count -= 4;
            }
            status = read32(self.addr + SMHC::STATUS);
            err = read32(self.addr + SMHC::RISR) & SDXC::INTERRUPT_ERROR_BIT;
        }

        loop {
            status = read32(self.addr + SMHC::RISR);

            err = status & SDXC::INTERRUPT_ERROR_BIT;
            if (blkcount > 1) {
                done = status & SDXC::AUTO_COMMAND_DONE;
            } else {
                done = status & SDXC::DATA_OVER;
            }
            if (done != 0 && err != 0) {
                break;
            }
        }

        if (err != 0 & SDXC::INTERRUPT_ERROR_BIT) {
            return false;
        }
        write32(self.addr + SMHC::RISR, 0xffffffff);

        if (count != 0) {
            return false;
        }
        return true;
    }

    pub fn init(&mut self) {
        let mut addr = 0u32;
        let mut val = 0u32;
        self.gpio_clk.init();
        self.gpio_cmd.init();
        self.gpio_d0.init();
        self.gpio_d1.init();
        self.gpio_d2.init();
        self.gpio_d3.init();

        //my
        addr = self.addr + SMHC::INTMASK;
        val = 0xFFFFFFCE;
        write32(addr, val);

        addr = 0x0200184c;
        val = read32(addr);
        val |= 1 << 16;
        write32(addr, val);

        /* Open the clock gate for sdhci0 */
        addr = 0x02001830;
        val = read32(addr);
        val |= (1 << 31) | (1 << 24) | 11; /* 50MHz */
        write32(addr, val);

        self.pclk = clk::get_peri1x_rate() / 12;

        /* sdhc0 clock gate pass */
        addr = 0x0200184c;
        val = read32(addr);
        val |= 1 << 0;
        write32(addr, val);
        udelay(10);

        write32(self.addr + SMHC::CTRL, SDXC::HARDWARE_RESET);
        write32(self.addr + SMHC::RISR, 0xffffffff);

        udelay(100);
    }
}
