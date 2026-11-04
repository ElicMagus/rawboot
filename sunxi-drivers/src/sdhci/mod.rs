// SPDX-License-Identifier: CC-BY-4.0

#![allow(dead_code)]
use crate::console::info;

use core::{
    mem::size_of,
    sync::atomic::{Ordering, fence},
};

use sunxi_pl::{
    cpu::{time_ms, time_us, udelay},
    gpio::{self, GpioMux},
    io::{read32, write32},
};

use crate::{
    ccu, clk,
    sdcard::{
        MMC_DATA_READ, MMC_DATA_WRITE, MMC_READ_MULTIPLE_BLOCK, MMC_RSP_136, MMC_RSP_BUSY,
        MMC_RSP_CRC, MMC_RSP_PRESENT, MMC_STOP_TRANSMISSION, MMC_WRITE_MULTIPLE_BLOCK,
    },
};

pub mod reg {
    pub const GCTRL: u32 = 0x00;
    pub const CLKCR: u32 = 0x04;
    pub const TIMEOUT: u32 = 0x08;
    pub const WIDTH: u32 = 0x0c;
    pub const BLKSZ: u32 = 0x10;
    pub const BYTECNT: u32 = 0x14;
    pub const CMD: u32 = 0x18;
    pub const ARG: u32 = 0x1c;
    pub const RESP0: u32 = 0x20;
    pub const RESP1: u32 = 0x24;
    pub const RESP2: u32 = 0x28;
    pub const RESP3: u32 = 0x2c;
    pub const IMASK: u32 = 0x30;
    pub const MINT: u32 = 0x34;
    pub const RINT: u32 = 0x38;
    pub const STATUS: u32 = 0x3c;
    pub const FTRGLEVEL: u32 = 0x40;
    pub const FUNCSEL: u32 = 0x44;
    pub const CBCR: u32 = 0x48;
    pub const BBCR: u32 = 0x4c;
    pub const DBGC: u32 = 0x50;
    pub const CSDC: u32 = 0x54;
    pub const A12A: u32 = 0x58;
    pub const NTSR: u32 = 0x5c;
    pub const HWRST: u32 = 0x78;
    pub const DMAC: u32 = 0x80;
    pub const DLBA: u32 = 0x84;
    pub const IDST: u32 = 0x88;
    pub const IDIE: u32 = 0x8c;
    pub const CHDA: u32 = 0x90;
    pub const CBDA: u32 = 0x94;
    pub const THLDC: u32 = 0x100;
    pub const SFC: u32 = 0x104;
    pub const DSBD: u32 = 0x10c;
    pub const DRV_DL: u32 = 0x140;
    pub const SAMP_DL: u32 = 0x144;
    pub const DS_DL: u32 = 0x148;
    pub const NTDC: u32 = 0x14c;
    pub const SKEW_DAT0_DL: u32 = 0x160;
    pub const SKEW_DAT1_DL: u32 = 0x164;
    pub const SKEW_DAT2_DL: u32 = 0x168;
    pub const SKEW_DAT3_DL: u32 = 0x16c;
    pub const SKEW_DAT4_DL: u32 = 0x170;
    pub const SKEW_DAT5_DL: u32 = 0x174;
    pub const SKEW_DAT6_DL: u32 = 0x178;
    pub const SKEW_DAT7_DL: u32 = 0x17c;
    pub const SKEW_DS_DL: u32 = 0x180;
    pub const SKEW_CTRL: u32 = 0x184;
    pub const FIFO: u32 = 0x200;
    pub const VERS: u32 = 0x300;
}

pub const MMC_CLK_400K: u32 = 0;
pub const MMC_CLK_25M: u32 = 1;
pub const MMC_CLK_50M: u32 = 2;
pub const MMC_CLK_50M_DDR: u32 = 3;
pub const MMC_CLK_100M: u32 = 4;
pub const MMC_CLK_150M: u32 = 5;
pub const MMC_CLK_200M: u32 = 6;

pub const SMHC_CLK_COUNT: usize = 7;

pub const MMC_VDD_27_36: u32 = 1 << 0;
pub const MMC_VDD_165_195: u32 = 1 << 1;

pub const MMC_BUS_WIDTH_1: u32 = 1;
pub const MMC_BUS_WIDTH_4: u32 = 2;

const SMHC_GCTRL_SOFT_RESET: u32 = 1 << 0;
const SMHC_GCTRL_FIFO_RESET: u32 = 1 << 1;
const SMHC_GCTRL_DMA_RESET: u32 = 1 << 2;
const SMHC_GCTRL_INTERRUPT_ENABLE: u32 = 1 << 4;
const SMHC_GCTRL_DMA_ENABLE: u32 = 1 << 5;
const SMHC_GCTRL_DEBOUNCE_ENABLE: u32 = 1 << 8;
const SMHC_GCTRL_POSEDGE_LATCH_DATA: u32 = 1 << 9;
const SMHC_GCTRL_DDR_MODE: u32 = 1 << 10;
const SMHC_GCTRL_MEMORY_ACCESS_DONE: u32 = 1 << 29;
const SMHC_GCTRL_ACCESS_DONE_DIRECT: u32 = 1 << 30;
const SMHC_GCTRL_ACCESS_BY_AHB: u32 = 1 << 31;
const SMHC_GCTRL_ACCESS_BY_DMA: u32 = 0 << 31;
const SMHC_GCTRL_HARDWARE_RESET: u32 =
    SMHC_GCTRL_SOFT_RESET | SMHC_GCTRL_FIFO_RESET | SMHC_GCTRL_DMA_RESET;

const SMHC_CLKCR_MASK_D0: u32 = 1 << 31;
const SMHC_CLKCR_CARD_CLOCK_ON: u32 = 1 << 16;
const SMHC_CLKCR_LOW_POWER_ON: u32 = 1 << 17;
const fn smhc_clkcr_clock_div(n: u32) -> u32 {
    (n - 1) & 0xff
}

const SMHC_WIDTH_1BIT: u32 = 0;
const SMHC_WIDTH_4BIT: u32 = 1;

const SMHC_CMD_RESP_EXPIRE: u32 = 1 << 6;
const SMHC_CMD_LONG_RESPONSE: u32 = 1 << 7;
const SMHC_CMD_CHECK_RESPONSE_CRC: u32 = 1 << 8;
const SMHC_CMD_DATA_EXPIRE: u32 = 1 << 9;
const SMHC_CMD_WRITE: u32 = 1 << 10;
const SMHC_CMD_SEQUENCE_MODE: u32 = 1 << 11;
const SMHC_CMD_SEND_AUTO_STOP: u32 = 1 << 12;
const SMHC_CMD_WAIT_PRE_OVER: u32 = 1 << 13;
const SMHC_CMD_STOP_ABORT_CMD: u32 = 1 << 14;
const SMHC_CMD_SEND_INIT_SEQUENCE: u32 = 1 << 15;
const SMHC_CMD_UPCLK_ONLY: u32 = 1 << 21;
const SMHC_CMD_READ_CEATA_DEV: u32 = 1 << 22;
const SMHC_CMD_CCS_EXPIRE: u32 = 1 << 23;
const SMHC_CMD_ENABLE_BIT_BOOT: u32 = 1 << 24;
const SMHC_CMD_ALT_BOOT_OPTIONS: u32 = 1 << 25;
const SMHC_CMD_BOOT_ACK_EXPIRE: u32 = 1 << 26;
const SMHC_CMD_BOOT_ABORT: u32 = 1 << 27;
const SMHC_CMD_VOLTAGE_SWITCH: u32 = 1 << 28;
const SMHC_CMD_USE_HOLD_REGISTER: u32 = 1 << 29;
const SMHC_CMD_START: u32 = 1 << 31;

const SMHC_RINT_RESP_ERROR: u32 = 0x1 << 1;
const SMHC_RINT_COMMAND_DONE: u32 = 0x1 << 2;
const SMHC_RINT_DATA_OVER: u32 = 0x1 << 3;
const SMHC_RINT_TX_DATA_REQUEST: u32 = 0x1 << 4;
const SMHC_RINT_RX_DATA_REQUEST: u32 = 0x1 << 5;
const SMHC_RINT_RESP_CRC_ERROR: u32 = 0x1 << 6;
const SMHC_RINT_DATA_CRC_ERROR: u32 = 0x1 << 7;
const SMHC_RINT_RESP_TIMEOUT: u32 = 0x1 << 8;
const SMHC_RINT_DATA_TIMEOUT: u32 = 0x1 << 9;
const SMHC_RINT_VOLTAGE_CHANGE_DONE: u32 = 0x1 << 10;
const SMHC_RINT_FIFO_RUN_ERROR: u32 = 0x1 << 11;
const SMHC_RINT_HARD_WARE_LOCKED: u32 = 0x1 << 12;
const SMHC_RINT_START_BIT_ERROR: u32 = 0x1 << 13;
const SMHC_RINT_AUTO_COMMAND_DONE: u32 = 0x1 << 14;
const SMHC_RINT_END_BIT_ERROR: u32 = 0x1 << 15;
const SMHC_RINT_SDIO_INTERRUPT: u32 = 0x1 << 16;
const SMHC_RINT_CARD_INSERT: u32 = 0x1 << 30;
const SMHC_RINT_CARD_REMOVE: u32 = 0x1 << 31;
const SMHC_RINT_INTERRUPT_ERROR_BIT: u32 = SMHC_RINT_RESP_ERROR
    | SMHC_RINT_RESP_CRC_ERROR
    | SMHC_RINT_DATA_CRC_ERROR
    | SMHC_RINT_RESP_TIMEOUT
    | SMHC_RINT_DATA_TIMEOUT
    | SMHC_RINT_VOLTAGE_CHANGE_DONE
    | SMHC_RINT_FIFO_RUN_ERROR
    | SMHC_RINT_HARD_WARE_LOCKED
    | SMHC_RINT_START_BIT_ERROR
    | SMHC_RINT_END_BIT_ERROR;
const SMHC_RINT_INTERRUPT_DONE_BIT: u32 = SMHC_RINT_AUTO_COMMAND_DONE
    | SMHC_RINT_DATA_OVER
    | SMHC_RINT_COMMAND_DONE
    | SMHC_RINT_VOLTAGE_CHANGE_DONE;

const SMHC_STATUS_RXWL_FLAG: u32 = 1 << 0;
const SMHC_STATUS_TXWL_FLAG: u32 = 1 << 1;
const SMHC_STATUS_FIFO_EMPTY: u32 = 1 << 2;
const SMHC_STATUS_FIFO_FULL: u32 = 1 << 3;
const SMHC_STATUS_CARD_PRESENT: u32 = 1 << 8;
const SMHC_STATUS_CARD_DATA_BUSY: u32 = 1 << 9;
const SMHC_STATUS_DATA_FSM_BUSY: u32 = 1 << 10;
const SMHC_STATUS_DMA_REQUEST: u32 = 1 << 31;
const SMHC_STATUS_FIFO_SIZE: u32 = 16;
const SMHC_STATUS_FIFO_LEVEL: fn(u32) -> u32 = |x| (x >> 17) & 0x3fff;

const SMHC_IDMAC_SOFT_RESET: u32 = 1 << 0;
const SMHC_IDMAC_FIX_BURST: u32 = 1 << 1;
const SMHC_IDMAC_IDMA_ON: u32 = 1 << 7;
const SMHC_IDMAC_REFETCH_DES: u32 = 1 << 31;

const SMHC_IDMAC_TRANSMIT_INTERRUPT: u32 = 1 << 0;
const SMHC_IDMAC_RECEIVE_INTERRUPT: u32 = 1 << 1;
const SMHC_IDMAC_FATAL_BUS_ERROR: u32 = 1 << 2;
const SMHC_IDMAC_DESTINATION_INVALID: u32 = 1 << 4;
const SMHC_IDMAC_CARD_ERROR_SUM: u32 = 1 << 5;
const SMHC_IDMAC_NORMAL_INTERRUPT_SUM: u32 = 1 << 8;
const SMHC_IDMAC_ABNORMAL_INTERRUPT_SUM: u32 = 1 << 9;
const SMHC_IDMAC_HOST_ABORT_INTERRUPT: u32 = 1 << 10;

const SMHC_IDMAC_DES0_DIC: u32 = 1 << 1;
const SMHC_IDMAC_DES0_LD: u32 = 1 << 2;
const SMHC_IDMAC_DES0_FD: u32 = 1 << 3;
const SMHC_IDMAC_DES0_CH: u32 = 1 << 4;
const SMHC_IDMAC_DES0_ER: u32 = 1 << 5;
const SMHC_IDMAC_DES0_CES: u32 = 1 << 30;
const SMHC_IDMAC_DES0_OWN: u32 = 1 << 31;

const SUNXI_MMC_TIMING_MODE_0: u32 = 0;
const SUNXI_MMC_TIMING_MODE_1: u32 = 1;
const SUNXI_MMC_TIMING_MODE_2: u32 = 2;
const SUNXI_MMC_TIMING_MODE_3: u32 = 3;
const SUNXI_MMC_TIMING_MODE_4: u32 = 4;
const SUNXI_MMC_TIMING_MODE_5: u32 = 5;

const MMC_CLK_SAMPLE_POINIT_MODE_0: u32 = 8;
const MMC_CLK_SAMPLE_POINIT_MODE_1: u32 = 3;
const MMC_CLK_SAMPLE_POINIT_MODE_2: u32 = 2;
const MMC_CLK_SAMPLE_POINIT_MODE_2_HS400: u32 = 64;
const MMC_CLK_SAMPLE_POINIT_MODE_3: u32 = 64;
const MMC_CLK_SAMPLE_POINIT_MODE_4: u32 = 64;
const MMC_CLK_SAMPLE_POINIT_MODE_5: u32 = 64;

const TM5_OUT_PH90: u8 = 0;
const TM5_OUT_PH180: u8 = 1;
const TM5_IN_PH90: u8 = 0;
const TM5_IN_PH180: u8 = 1;
const TM5_IN_PH270: u8 = 2;
const TM5_IN_PH0: u8 = 3;

const SDXC_NTDC_START_CAL: u32 = 1 << 15;
const SDXC_NTDC_CAL_DONE: u32 = 1 << 14;
const SDXC_NTDC_CAL_DLY: u32 = 0x3f << 8;
const SDXC_NTDC_ENABLE_DLY: u32 = 1 << 7;
const SDXC_NTDC_CFG_DLY: u32 = 0x3f;
const SDXC_NTDC_CFG_NEW_DLY: u32 = 0xf;

const DTO_MAX: u32 = 200;
const SDXC_CAL_START: u32 = 1 << 15;
const SDXC_CAL_DONE: u32 = 1 << 14;
const SDXC_CAL_DL_SHIFT: u32 = 8;
const SDXC_CAL_DL_MASK: u32 = 0x3f;
const SDXC_CAL_DL_SW_SHIFT: u32 = 0;
const SDXC_CAL_DL_SW_EN: u32 = 1 << 7;
const SDXC_CAL_TIMEOUT_MS: u64 = 50;
const SUNXI_MMC_NTSR_MODE_SEL_NEW: u32 = 0x1 << 31;

const CONFIG_COUNTER_FREQUENCY: u32 = 24_000_000;

const SMHC_DES_NUM_SHIFT: u32 = 12;
const SMHC_DES_BUFFER_MAX_LEN: u32 = 1 << SMHC_DES_NUM_SHIFT;

#[derive(Default, Clone, Copy)]
pub struct SdhciCmd {
    pub idx: u32,
    pub arg: u32,
    pub resptype: u32,
    pub response: [u32; 4],
}

#[derive(Clone, Copy)]
pub struct SdhciData {
    pub buf: *mut u8,
    pub flag: u32,
    pub blksz: u32,
    pub blkcnt: u32,
}

#[repr(C, align(8))]
#[derive(Default, Clone, Copy)]
pub struct SdhciIdmaDesc {
    pub control: u32,
    pub data_buf_sz: u32,
    pub buf_addr: u32,
    pub next_desc_addr: u32,
}

pub struct Sdhci {
    pub name: &'static str,
    pub reg: u32,
    pub id: u32,
    pub reset: u32,
    pub voltage: u32,
    pub width: u32,
    pub clock_active: u32,
    pub clock_wanted: u32,
    pub pclk: u32,
    pub odly: [u8; SMHC_CLK_COUNT],
    pub sdly: [u8; SMHC_CLK_COUNT],
    pub dma_desc: [SdhciIdmaDesc; 32],
    pub dma_trglvl: u32,
    pub removable: bool,
    pub isspi: bool,
    gpio_d0: GpioMux,
    gpio_d1: GpioMux,
    gpio_d2: GpioMux,
    gpio_d3: GpioMux,
    gpio_cmd: GpioMux,
    gpio_clk: GpioMux,
}

impl Default for Sdhci {
    fn default() -> Self {
        Self {
            name: "sdhci0",
            reg: 0x0402_0000,
            id: 0,
            reset: 0,
            voltage: MMC_VDD_27_36,
            width: MMC_BUS_WIDTH_4,
            clock_active: MMC_CLK_400K,
            clock_wanted: MMC_CLK_50M,
            pclk: 0,
            odly: [0; SMHC_CLK_COUNT],
            sdly: [0; SMHC_CLK_COUNT],
            dma_desc: [SdhciIdmaDesc::default(); 32],
            dma_trglvl: 0,
            removable: false,
            isspi: false,
            gpio_d0: GpioMux {
                pin: gpio::gpio_pin(gpio::PORTF, 1),
                mux: gpio::GPIO_PERIPH_MUX2,
            },
            gpio_d1: GpioMux {
                pin: gpio::gpio_pin(gpio::PORTF, 0),
                mux: gpio::GPIO_PERIPH_MUX2,
            },
            gpio_d2: GpioMux {
                pin: gpio::gpio_pin(gpio::PORTF, 5),
                mux: gpio::GPIO_PERIPH_MUX2,
            },
            gpio_d3: GpioMux {
                pin: gpio::gpio_pin(gpio::PORTF, 4),
                mux: gpio::GPIO_PERIPH_MUX2,
            },
            gpio_cmd: GpioMux {
                pin: gpio::gpio_pin(gpio::PORTF, 3),
                mux: gpio::GPIO_PERIPH_MUX2,
            },
            gpio_clk: GpioMux {
                pin: gpio::gpio_pin(gpio::PORTF, 2),
                mux: gpio::GPIO_PERIPH_MUX2,
            },
        }
    }
}

struct SmhcCcuParams {
    clk_cfg: u32,
    gate_mask: u32,
    reset_mask: u32,
}

fn smhc_get_ccu_params(id: u32) -> Option<SmhcCcuParams> {
    let (clk_reg, gate_mask, reset_mask) = match id {
        0 => (
            ccu::SMHC0_CLK_REG,
            ccu::MMC_BGR_SMHC0_GATE,
            ccu::MMC_BGR_SMHC0_RST,
        ),
        1 => (
            ccu::SMHC1_CLK_REG,
            ccu::MMC_BGR_SMHC1_GATE,
            ccu::MMC_BGR_SMHC1_RST,
        ),
        2 => (
            ccu::SMHC2_CLK_REG,
            ccu::MMC_BGR_SMHC2_GATE,
            ccu::MMC_BGR_SMHC2_RST,
        ),
        _ => return None,
    };

    Some(SmhcCcuParams {
        clk_cfg: ccu::BASE + clk_reg,
        gate_mask,
        reset_mask,
    })
}

fn sdhci_configure_start_bit_detection(sdhci: &Sdhci, hs400_mode: bool) {
    let mut val = read32(sdhci.reg + reg::DSBD);

    val &= !((1 << 31) | (1 << 0));
    if hs400_mode {
        val |= 1 << 31;
    }
    val |= 1 << 0;

    write32(sdhci.reg + reg::DSBD, val);
}

fn set_read_timeout(sdhci: &Sdhci, timeout: u32) {
    let mut rdto_clk = sdhci.clock_active / 1000 * timeout;
    let mut rval = read32(sdhci.reg + reg::NTSR);
    let mode_2x = rval & (0x1 << 31);

    if sdhci.clock_active == MMC_CLK_50M_DDR && mode_2x != 0 {
        rdto_clk <<= 1;
    }

    rval = read32(sdhci.reg + reg::GCTRL);
    if rdto_clk > 0x00ff_ffff && sdhci.clock_active == MMC_CLK_50M_DDR {
        rdto_clk = rdto_clk.div_ceil(256);
        rval |= 0x1 << 11;
    } else {
        rdto_clk = 0x00ff_ffff;
        rval &= !(0x1 << 11);
    }
    write32(sdhci.reg + reg::GCTRL, rval);

    rval = read32(sdhci.reg + reg::TIMEOUT);
    rval &= !(0x00ff_ffff << 8);
    rval |= rdto_clk << 8;
    write32(sdhci.reg + reg::TIMEOUT, rval);
}

fn prepare_dma(sdhci: &mut Sdhci, data: &SdhciData) -> bool {
    if data.buf.is_null() {
        return false;
    }

    let byte_cnt = data.blksz * data.blkcnt;
    let mut buff_frag_num = byte_cnt >> SMHC_DES_NUM_SHIFT;
    let mut remain = byte_cnt & (SMHC_DES_BUFFER_MAX_LEN - 1);
    if remain != 0 {
        buff_frag_num += 1;
    } else {
        remain = SMHC_DES_BUFFER_MAX_LEN - 1;
    }

    if buff_frag_num as usize > sdhci.dma_desc.len() {
        return false;
    }

    let base = sdhci.dma_desc.as_ptr() as u32;
    let buf = data.buf as u32;

    for i in 0..buff_frag_num as usize {
        let next = if i + 1 < buff_frag_num as usize {
            base + (i + 1) as u32 * size_of::<SdhciIdmaDesc>() as u32
        } else {
            0
        };
        let mut desc = SdhciIdmaDesc {
            control: SMHC_IDMAC_DES0_CH | SMHC_IDMAC_DES0_OWN | SMHC_IDMAC_DES0_DIC,
            ..Default::default()
        };
        if buff_frag_num > 1 && i as u32 != buff_frag_num - 1 {
            desc.data_buf_sz = SMHC_DES_BUFFER_MAX_LEN - 1;
        } else {
            desc.data_buf_sz = remain;
        }
        desc.buf_addr = (buf + i as u32 * SMHC_DES_BUFFER_MAX_LEN) >> 2;
        if i == 0 {
            desc.control |= SMHC_IDMAC_DES0_FD;
        }
        if i + 1 == buff_frag_num as usize {
            desc.control &= !SMHC_IDMAC_DES0_DIC;
            desc.control |= SMHC_IDMAC_DES0_LD;
            desc.next_desc_addr = 0;
        } else {
            desc.next_desc_addr = next >> 2;
        }
        sdhci.dma_desc[i] = desc;
    }

    fence(Ordering::SeqCst);

    write32(sdhci.reg + reg::IDST, 0x337);
    write32(
        sdhci.reg + reg::GCTRL,
        read32(sdhci.reg + reg::GCTRL) | SMHC_GCTRL_DMA_ENABLE | SMHC_GCTRL_DMA_RESET,
    );
    write32(sdhci.reg + reg::DMAC, SMHC_IDMAC_SOFT_RESET);
    while read32(sdhci.reg + reg::DMAC) & SMHC_IDMAC_SOFT_RESET != 0 {}

    write32(
        sdhci.reg + reg::DMAC,
        SMHC_IDMAC_FIX_BURST | SMHC_IDMAC_IDMA_ON,
    );
    let mut idie = read32(sdhci.reg + reg::IDIE);
    idie &= !(SMHC_IDMAC_TRANSMIT_INTERRUPT | SMHC_IDMAC_RECEIVE_INTERRUPT);
    if data.flag & MMC_DATA_WRITE != 0 {
        idie |= SMHC_IDMAC_TRANSMIT_INTERRUPT;
    } else {
        idie |= SMHC_IDMAC_RECEIVE_INTERRUPT;
    }
    write32(sdhci.reg + reg::IDIE, idie);

    write32(sdhci.reg + reg::DLBA, base >> 2);
    write32(sdhci.reg + reg::FTRGLEVEL, sdhci.dma_trglvl);

    true
}

/// The IDMAC completion bit for this transfer direction.
fn idmac_done_interrupt(flag: u32) -> u32 {
    if flag & MMC_DATA_WRITE != 0 {
        SMHC_IDMAC_TRANSMIT_INTERRUPT
    } else {
        SMHC_IDMAC_RECEIVE_INTERRUPT
    }
}

fn wait_done(
    sdhci: &Sdhci,
    dat: Option<&SdhciData>,
    timeout_msecs: u32,
    flag: u32,
    dma: bool,
) -> Result<u32, u32> {
    let start = time_ms();

    loop {
        let status = read32(sdhci.reg + reg::RINT);
        if time_ms().wrapping_sub(start) > timeout_msecs
            || status & SMHC_RINT_INTERRUPT_ERROR_BIT != 0
        {
            return Err(status);
        }

        let done = match dat {
            Some(dat) if dma && dat.blkcnt * dat.blksz > 0 => {
                status & flag != 0
                    && read32(sdhci.reg + reg::IDST) & idmac_done_interrupt(dat.flag) != 0
            }
            _ => status & flag != 0,
        };
        if done {
            return Ok(status);
        }
    }
}

fn read_bytes(sdhci: &Sdhci, dat: &SdhciData) -> bool {
    let mut count = dat.blkcnt * dat.blksz;
    let mut tmp = dat.buf;
    let mut timeout = time_ms() + count;
    if timeout < 250 {
        timeout = 250;
    }

    let mut status = read32(sdhci.reg + reg::STATUS);
    let mut err = read32(sdhci.reg + reg::RINT) & SMHC_RINT_INTERRUPT_ERROR_BIT;
    if err != 0 {
        info!(
            "SMHC: interrupt error 0x{:x} status 0x{:x}\r\n",
            err & SMHC_RINT_INTERRUPT_ERROR_BIT,
            status
        );
    }

    while err == 0 && count >= size_of::<u32>() as u32 {
        while read32(sdhci.reg + reg::STATUS) & SMHC_STATUS_FIFO_EMPTY != 0 {
            if time_ms() > timeout {
                info!("SMHC: read timeout\r\n");
                return false;
            }
        }

        status = read32(sdhci.reg + reg::STATUS);
        let mut in_fifo = SMHC_STATUS_FIFO_LEVEL(status).min(count / size_of::<u32>() as u32);
        count -= size_of::<u32>() as u32 * in_fifo;
        while in_fifo != 0 {
            unsafe {
                core::ptr::write_unaligned(tmp as *mut u32, read32(sdhci.reg + reg::FIFO));
                tmp = tmp.add(size_of::<u32>());
            }
            in_fifo -= 1;
        }

        err = read32(sdhci.reg + reg::RINT) & SMHC_RINT_INTERRUPT_ERROR_BIT;
    }

    let mut done;
    loop {
        status = read32(sdhci.reg + reg::RINT);
        err = status & SMHC_RINT_INTERRUPT_ERROR_BIT;
        done = if dat.blkcnt > 1 {
            status & SMHC_RINT_AUTO_COMMAND_DONE
        } else {
            status & SMHC_RINT_DATA_OVER
        };
        if done != 0 || err != 0 {
            break;
        }
    }

    if err & SMHC_RINT_INTERRUPT_ERROR_BIT != 0 {
        info!(
            "SMHC: interrupt error 0x{:x} status 0x{:x}\r\n",
            err & SMHC_RINT_INTERRUPT_ERROR_BIT,
            status
        );
        return false;
    }
    if count != 0 {
        info!("SMHC: read {} leftover\r\n", count);
        return false;
    }

    true
}

fn write_bytes(sdhci: &Sdhci, dat: &SdhciData) -> bool {
    let mut count = dat.blkcnt * dat.blksz;
    let mut tmp = dat.buf;
    let mut timeout = time_ms() + count;
    if timeout < 250 {
        timeout = 250;
    }

    let mut status;
    let mut err = read32(sdhci.reg + reg::RINT) & SMHC_RINT_INTERRUPT_ERROR_BIT;
    while err == 0 && count != 0 {
        while read32(sdhci.reg + reg::STATUS) & SMHC_STATUS_FIFO_FULL != 0 {
            if time_ms() > timeout {
                info!("SMHC: write timeout\r\n");
                return false;
            }
        }

        unsafe {
            write32(
                sdhci.reg + reg::FIFO,
                core::ptr::read_unaligned(tmp as *const u32),
            );
            tmp = tmp.add(size_of::<u32>());
        }
        count -= size_of::<u32>() as u32;

        err = read32(sdhci.reg + reg::RINT) & SMHC_RINT_INTERRUPT_ERROR_BIT;
    }

    let mut done;
    loop {
        status = read32(sdhci.reg + reg::RINT);
        err = status & SMHC_RINT_INTERRUPT_ERROR_BIT;
        done = if dat.blkcnt > 1 {
            status & SMHC_RINT_AUTO_COMMAND_DONE
        } else {
            status & SMHC_RINT_DATA_OVER
        };
        if done != 0 || err != 0 {
            break;
        }
    }

    if err & SMHC_RINT_INTERRUPT_ERROR_BIT != 0 {
        return false;
    }
    if count != 0 {
        return false;
    }

    true
}

pub fn sdhci_reset(sdhci: &mut Sdhci) -> bool {
    write32(sdhci.reg + reg::GCTRL, SMHC_GCTRL_HARDWARE_RESET);
    true
}

pub fn sdhci_set_voltage(_sdhci: &mut Sdhci, _voltage: u32) -> bool {
    false
}

pub fn sdhci_set_width(sdhci: &mut Sdhci, width: u32) -> bool {
    let mut gctrl = read32(sdhci.reg + reg::GCTRL);
    let enable_ddr = match width {
        MMC_BUS_WIDTH_1 => {
            write32(sdhci.reg + reg::WIDTH, SMHC_WIDTH_1BIT);
            false
        }
        MMC_BUS_WIDTH_4 => {
            write32(sdhci.reg + reg::WIDTH, SMHC_WIDTH_4BIT);
            sdhci.clock_active == MMC_CLK_50M_DDR
        }
        _ => {
            info!("SMHC: {} width value invalid\r\n", width);
            return false;
        }
    };

    if enable_ddr {
        gctrl |= SMHC_GCTRL_DDR_MODE | SMHC_GCTRL_POSEDGE_LATCH_DATA;
    } else {
        gctrl &= !SMHC_GCTRL_DDR_MODE;
    }
    write32(sdhci.reg + reg::GCTRL, gctrl);

    sdhci_configure_start_bit_detection(sdhci, false);

    if enable_ddr {
        config_delay(sdhci);
    }

    true
}

fn init_default_timing(sdhci: &mut Sdhci) {
    sdhci.odly[MMC_CLK_400K as usize] = TM5_OUT_PH180;
    sdhci.odly[MMC_CLK_25M as usize] = TM5_OUT_PH180;
    sdhci.odly[MMC_CLK_50M as usize] = TM5_OUT_PH180;
    sdhci.odly[MMC_CLK_50M_DDR as usize] = TM5_OUT_PH90;
    sdhci.odly[MMC_CLK_100M as usize] = TM5_OUT_PH90;
    sdhci.odly[MMC_CLK_150M as usize] = TM5_OUT_PH90;
    sdhci.odly[MMC_CLK_200M as usize] = TM5_OUT_PH90;

    sdhci.sdly[MMC_CLK_400K as usize] = TM5_IN_PH180;
    sdhci.sdly[MMC_CLK_25M as usize] = TM5_IN_PH180;
    sdhci.sdly[MMC_CLK_50M as usize] = TM5_IN_PH90;
    sdhci.sdly[MMC_CLK_50M_DDR as usize] = TM5_IN_PH180;
    sdhci.sdly[MMC_CLK_100M as usize] = TM5_IN_PH90;
    sdhci.sdly[MMC_CLK_150M as usize] = TM5_IN_PH90;
    sdhci.sdly[MMC_CLK_200M as usize] = TM5_IN_PH90;
}

fn config_delay(sdhci: &mut Sdhci) -> bool {
    let params = match smhc_get_ccu_params(sdhci.id) {
        Some(params) => params,
        None => {
            info!("SMHC: unsupported controller id {}\r\n", sdhci.id);
            return false;
        }
    };

    let freq = sdhci.clock_active;
    if freq as usize >= SMHC_CLK_COUNT {
        info!("SMHC: invalid timing index {}\r\n", freq);
        return false;
    }

    let odly = sdhci.odly[freq as usize];
    let sdly = sdhci.sdly[freq as usize];

    write32(
        params.clk_cfg,
        read32(params.clk_cfg) & !ccu::MMC_CTRL_ENABLE,
    );
    let mut drv_dl = read32(sdhci.reg + reg::DRV_DL);
    drv_dl &= !(0x3 << 16);
    drv_dl |= ((odly as u32 & 0x1) << 16) | ((odly as u32 & 0x1) << 17);
    write32(sdhci.reg + reg::DRV_DL, drv_dl);
    write32(
        params.clk_cfg,
        read32(params.clk_cfg) | ccu::MMC_CTRL_ENABLE,
    );

    let mut rval = read32(sdhci.reg + reg::NTSR);
    rval &= !(0x3 << 8);
    rval |= (sdly as u32 & 0x3) << 8;
    write32(sdhci.reg + reg::NTSR, rval);

    let mut rval = read32(sdhci.reg + reg::SKEW_CTRL);
    rval |= 0x1 << 4;
    write32(sdhci.reg + reg::SKEW_CTRL, rval);

    if freq > MMC_CLK_400K {
        let mut calib = read32(sdhci.reg + reg::SAMP_DL);
        calib &= !SDXC_CAL_DL_SW_EN;
        calib &= !((SDXC_CAL_DL_MASK << SDXC_CAL_DL_SW_SHIFT) | SDXC_CAL_START | SDXC_CAL_DONE);
        write32(sdhci.reg + reg::SAMP_DL, calib | SDXC_CAL_START);

        let timeout = time_us();
        loop {
            calib = read32(sdhci.reg + reg::SAMP_DL);
            if time_us() - timeout > SDXC_CAL_TIMEOUT_MS * 1000 {
                info!("SMHC: sample delay calibration timeout\r\n");
                write32(sdhci.reg + reg::SAMP_DL, SDXC_CAL_DL_SW_EN);
                return false;
            }
            if calib & SDXC_CAL_DONE != 0 {
                break;
            }
        }

        let mut delay = (calib >> SDXC_CAL_DL_SHIFT) & SDXC_CAL_DL_MASK;
        if delay == 0 {
            delay = 1;
        }

        calib &= !SDXC_CAL_START;
        calib &= !(SDXC_CAL_DL_MASK << SDXC_CAL_DL_SW_SHIFT);
        calib &= !SDXC_CAL_DL_SW_EN;
        calib |= (delay << SDXC_CAL_DL_SW_SHIFT) | SDXC_CAL_DL_SW_EN;
        write32(sdhci.reg + reg::SAMP_DL, calib);
    }

    true
}

fn update_card_clock(sdhci: &Sdhci) -> bool {
    write32(
        sdhci.reg + reg::CMD,
        SMHC_CMD_START | SMHC_CMD_UPCLK_ONLY | SMHC_CMD_WAIT_PRE_OVER,
    );
    let timeout = time_ms();

    loop {
        if time_ms().wrapping_sub(timeout) > 10 {
            return false;
        }
        if read32(sdhci.reg + reg::CMD) & SMHC_CMD_START == 0 {
            break;
        }
    }

    write32(sdhci.reg + reg::RINT, 0xffff_ffff);
    true
}

pub fn sdhci_set_clock(sdhci: &mut Sdhci, clock: u32) -> bool {
    let params = match smhc_get_ccu_params(sdhci.id) {
        Some(params) => params,
        None => {
            info!("SMHC: unsupported controller id {}\r\n", sdhci.id);
            return false;
        }
    };

    sdhci.clock_active = clock;
    let is_ddr = clock == MMC_CLK_50M_DDR;

    let hz = match clock {
        MMC_CLK_400K => 400_000,
        MMC_CLK_25M => 25_000_000,
        MMC_CLK_50M | MMC_CLK_50M_DDR => 50_000_000,
        MMC_CLK_100M => 100_000_000,
        MMC_CLK_150M => 150_000_000,
        MMC_CLK_200M => 200_000_000,
        _ => {
            info!("SHMC: invalid clock: {}\r\n", clock);
            return false;
        }
    };

    let mod_hz = if is_ddr { hz * 4 } else { hz * 2 };

    let (pll, pll_hz) = if mod_hz <= CONFIG_COUNTER_FREQUENCY {
        (ccu::MMC_CTRL_OSCM24, CONFIG_COUNTER_FREQUENCY)
    } else {
        (ccu::MMC_CTRL_PLL_PERIPH1X, clk::sunxi_clk_get_peri1x_rate())
    };

    let mut div = pll_hz / mod_hz;
    if pll_hz % mod_hz != 0 {
        div += 1;
    }

    let mut n = 0;
    while div > 16 {
        n += 1;
        div = div.div_ceil(2);
    }

    if n > 3 {
        info!("mmc {} error cannot set clock to {}Hz\r\n", sdhci.name, hz);
        return false;
    }

    write32(
        sdhci.reg + reg::CLKCR,
        read32(sdhci.reg + reg::CLKCR) & !SMHC_CLKCR_CARD_CLOCK_ON,
    );
    if !update_card_clock(sdhci) {
        return false;
    }

    write32(
        sdhci.reg + reg::NTSR,
        read32(sdhci.reg + reg::NTSR) | SUNXI_MMC_NTSR_MODE_SEL_NEW,
    );

    let clk_ctrl = pll | ccu::mmc_ctrl_n(n) | ccu::mmc_ctrl_m(div) | ccu::MMC_CTRL_NEW_TIMING;

    if params.gate_mask != 0 || params.reset_mask != 0 {
        let mut gate_ctrl = read32(ccu::BASE + ccu::SMHC_BGR_REG);
        if params.gate_mask != 0 {
            gate_ctrl &= !params.gate_mask;
            write32(ccu::BASE + ccu::SMHC_BGR_REG, gate_ctrl);
        }
        if params.reset_mask != 0 {
            let mut reset_ctrl = gate_ctrl & !params.reset_mask;
            write32(ccu::BASE + ccu::SMHC_BGR_REG, reset_ctrl);
            udelay(2);
            reset_ctrl |= params.reset_mask;
            write32(ccu::BASE + ccu::SMHC_BGR_REG, reset_ctrl);
            gate_ctrl = reset_ctrl;
        }

        write32(
            params.clk_cfg,
            read32(params.clk_cfg) & !ccu::MMC_CTRL_ENABLE,
        );
        write32(params.clk_cfg, clk_ctrl);
        write32(
            params.clk_cfg,
            read32(params.clk_cfg) | ccu::MMC_CTRL_ENABLE,
        );

        if params.gate_mask != 0 {
            gate_ctrl |= params.gate_mask;
            write32(ccu::BASE + ccu::SMHC_BGR_REG, gate_ctrl);
        }
    } else {
        write32(
            params.clk_cfg,
            read32(params.clk_cfg) & !ccu::MMC_CTRL_ENABLE,
        );
        write32(params.clk_cfg, clk_ctrl);
        write32(
            params.clk_cfg,
            read32(params.clk_cfg) | ccu::MMC_CTRL_ENABLE,
        );
    }

    sdhci.pclk = mod_hz;

    write32(
        sdhci.reg + reg::CLKCR,
        read32(sdhci.reg + reg::CLKCR) | SMHC_CLKCR_MASK_D0,
    );
    write32(
        sdhci.reg + reg::CLKCR,
        read32(sdhci.reg + reg::CLKCR) & !0xff,
    );
    if is_ddr {
        write32(
            sdhci.reg + reg::CLKCR,
            read32(sdhci.reg + reg::CLKCR) | smhc_clkcr_clock_div(2),
        );
    }
    write32(
        sdhci.reg + reg::CLKCR,
        read32(sdhci.reg + reg::CLKCR) | SMHC_CLKCR_CARD_CLOCK_ON,
    );

    if !update_card_clock(sdhci) {
        return false;
    }

    write32(
        sdhci.reg + reg::CLKCR,
        read32(sdhci.reg + reg::CLKCR) & !SMHC_CLKCR_MASK_D0,
    );

    config_delay(sdhci)
}

pub fn sdhci_transfer(sdhci: &mut Sdhci, cmd: &mut SdhciCmd, dat: Option<&SdhciData>) -> bool {
    let mut cmdval = 0u32;
    let mut status;
    let timeout;

    if cmd.idx == MMC_STOP_TRANSMISSION {
        timeout = time_ms();
        loop {
            status = read32(sdhci.reg + reg::STATUS);
            if time_ms().wrapping_sub(timeout) > 10 {
                write32(sdhci.reg + reg::GCTRL, SMHC_GCTRL_HARDWARE_RESET);
                write32(sdhci.reg + reg::RINT, 0xffff_ffff);
                info!("SMHC: stop timeout\r\n");
                return false;
            }
            if status & SMHC_STATUS_CARD_DATA_BUSY == 0 {
                break;
            }
        }
        return true;
    }

    if cmd.resptype & MMC_RSP_PRESENT != 0 {
        cmdval |= SMHC_CMD_RESP_EXPIRE;
        if cmd.resptype & MMC_RSP_136 != 0 {
            cmdval |= SMHC_CMD_LONG_RESPONSE;
        }
        if cmd.resptype & MMC_RSP_CRC != 0 {
            cmdval |= SMHC_CMD_CHECK_RESPONSE_CRC;
        }
    }

    if sdhci.clock_active >= MMC_CLK_50M {
        cmdval |= SMHC_CMD_USE_HOLD_REGISTER;
    }

    if let Some(dat) = dat {
        write32(sdhci.reg + reg::BLKSZ, dat.blksz);
        write32(sdhci.reg + reg::BYTECNT, dat.blkcnt * dat.blksz);

        cmdval |= SMHC_CMD_DATA_EXPIRE | SMHC_CMD_WAIT_PRE_OVER;
        set_read_timeout(sdhci, DTO_MAX);

        if dat.flag & MMC_DATA_WRITE != 0 {
            cmdval |= SMHC_CMD_WRITE;
            write32(
                sdhci.reg + reg::IDST,
                read32(sdhci.reg + reg::IDST) | SMHC_IDMAC_TRANSMIT_INTERRUPT,
            );
        }
        if dat.flag & MMC_DATA_READ != 0 {
            write32(
                sdhci.reg + reg::IDST,
                read32(sdhci.reg + reg::IDST) | SMHC_IDMAC_RECEIVE_INTERRUPT,
            );
        }
    }

    if cmd.idx == MMC_WRITE_MULTIPLE_BLOCK || cmd.idx == MMC_READ_MULTIPLE_BLOCK {
        cmdval |= SMHC_CMD_SEND_AUTO_STOP;
    }

    write32(sdhci.reg + reg::RINT, 0xffff_ffff);
    write32(sdhci.reg + reg::ARG, cmd.arg);

    let mut dma = false;
    if let Some(dat) = dat {
        let dlen = dat.blkcnt * dat.blksz;
        if dlen > 64 {
            dma = true;
            write32(
                sdhci.reg + reg::GCTRL,
                read32(sdhci.reg + reg::GCTRL) & !SMHC_GCTRL_ACCESS_BY_AHB,
            );
            if !prepare_dma(sdhci, dat) {
                return false;
            }
            write32(sdhci.reg + reg::CMD, cmdval | cmd.idx | SMHC_CMD_START);
        } else if dlen > 0 {
            write32(
                sdhci.reg + reg::GCTRL,
                read32(sdhci.reg + reg::GCTRL) | SMHC_GCTRL_ACCESS_BY_AHB,
            );
            write32(sdhci.reg + reg::CMD, cmdval | cmd.idx | SMHC_CMD_START);
            if (dat.flag & MMC_DATA_READ != 0 && !read_bytes(sdhci, dat))
                || (dat.flag & MMC_DATA_WRITE != 0 && !write_bytes(sdhci, dat))
            {
                return false;
            }
        } else {
            write32(
                sdhci.reg + reg::GCTRL,
                read32(sdhci.reg + reg::GCTRL) | SMHC_GCTRL_ACCESS_BY_AHB,
            );
            write32(sdhci.reg + reg::CMD, cmdval | cmd.idx | SMHC_CMD_START);
        }
    } else {
        write32(
            sdhci.reg + reg::GCTRL,
            read32(sdhci.reg + reg::GCTRL) | SMHC_GCTRL_ACCESS_BY_AHB,
        );
        write32(sdhci.reg + reg::CMD, cmdval | cmd.idx | SMHC_CMD_START);
    }

    if let Err(status) = wait_done(sdhci, None, 100, SMHC_RINT_COMMAND_DONE, false) {
        info!(
            "SMHC: cmd{} timeout (rint=0x{:08x}, flag=0x{:08x})\r\n",
            cmd.idx, status, SMHC_RINT_COMMAND_DONE
        );
        return false;
    }

    if let Some(dat) = dat {
        let flag = if dat.blkcnt > 1 {
            SMHC_RINT_AUTO_COMMAND_DONE
        } else {
            SMHC_RINT_DATA_OVER
        };
        if let Err(status) = wait_done(sdhci, Some(dat), 6000, flag, dma) {
            info!(
                "SMHC: data timeout on cmd{} (rint=0x{:08x}, flag=0x{:08x}, idst=0x{:08x})\r\n",
                cmd.idx,
                status,
                flag,
                read32(sdhci.reg + reg::IDST)
            );
            return false;
        }
    }

    if cmd.resptype & MMC_RSP_BUSY != 0 {
        timeout = time_ms();
        loop {
            status = read32(sdhci.reg + reg::STATUS);
            if time_ms().wrapping_sub(timeout) > 10 {
                write32(sdhci.reg + reg::GCTRL, SMHC_GCTRL_HARDWARE_RESET);
                write32(sdhci.reg + reg::RINT, 0xffff_ffff);
                info!("SMHC: busy timeout\r\n");
                return false;
            }
            if status & SMHC_STATUS_CARD_DATA_BUSY == 0 {
                break;
            }
        }
    }

    if cmd.resptype & MMC_RSP_136 != 0 {
        cmd.response[0] = read32(sdhci.reg + reg::RESP3);
        cmd.response[1] = read32(sdhci.reg + reg::RESP2);
        cmd.response[2] = read32(sdhci.reg + reg::RESP1);
        cmd.response[3] = read32(sdhci.reg + reg::RESP0);
    } else {
        cmd.response[0] = read32(sdhci.reg + reg::RESP0);
    }

    if dma && dat.is_some() {
        status = read32(sdhci.reg + reg::IDST);
        write32(sdhci.reg + reg::IDST, status);
        write32(sdhci.reg + reg::IDIE, 0);
        write32(sdhci.reg + reg::DMAC, 0);
        write32(
            sdhci.reg + reg::GCTRL,
            read32(sdhci.reg + reg::GCTRL) & !SMHC_GCTRL_DMA_ENABLE,
        );
    }

    true
}

pub fn sunxi_sdhci_init(sdhci: &mut Sdhci) {
    gpio::sunxi_gpio_init(sdhci.gpio_clk.pin, sdhci.gpio_clk.mux);
    gpio::sunxi_gpio_set_pull(sdhci.gpio_clk.pin, gpio::GpioPull::Up);

    gpio::sunxi_gpio_init(sdhci.gpio_cmd.pin, sdhci.gpio_cmd.mux);
    gpio::sunxi_gpio_set_pull(sdhci.gpio_cmd.pin, gpio::GpioPull::Up);

    gpio::sunxi_gpio_init(sdhci.gpio_d0.pin, sdhci.gpio_d0.mux);
    gpio::sunxi_gpio_set_pull(sdhci.gpio_d0.pin, gpio::GpioPull::Up);

    gpio::sunxi_gpio_init(sdhci.gpio_d1.pin, sdhci.gpio_d1.mux);
    gpio::sunxi_gpio_set_pull(sdhci.gpio_d1.pin, gpio::GpioPull::Up);

    gpio::sunxi_gpio_init(sdhci.gpio_d2.pin, sdhci.gpio_d2.mux);
    gpio::sunxi_gpio_set_pull(sdhci.gpio_d2.pin, gpio::GpioPull::Up);

    gpio::sunxi_gpio_init(sdhci.gpio_d3.pin, sdhci.gpio_d3.mux);
    gpio::sunxi_gpio_set_pull(sdhci.gpio_d3.pin, gpio::GpioPull::Up);

    init_default_timing(sdhci);

    write32(sdhci.reg + reg::GCTRL, SMHC_GCTRL_HARDWARE_RESET);
    write32(sdhci.reg + reg::RINT, 0xffff_ffff);

    sdhci.dma_trglvl = (0x3 << 28) | (15 << 16) | 240;

    udelay(100);

    sdhci_configure_start_bit_detection(sdhci, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_waits_for_the_transmit_interrupt() {
        assert_eq!(
            idmac_done_interrupt(MMC_DATA_WRITE),
            SMHC_IDMAC_TRANSMIT_INTERRUPT
        );
    }

    #[test]
    fn a_read_waits_for_the_receive_interrupt() {
        assert_eq!(
            idmac_done_interrupt(MMC_DATA_READ),
            SMHC_IDMAC_RECEIVE_INTERRUPT
        );
    }
}
