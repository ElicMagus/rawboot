use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    ccu,
    io::{read32, sdelay, write32},
};

const CLK_WAIT_TIMEOUT: u32 = 2_000_000;

static CLK_WAIT_FAIL_ADDR: AtomicU32 = AtomicU32::new(0);

fn wait_for_bit_set(addr: u32, mask: u32) -> bool {
    let mut timeout = CLK_WAIT_TIMEOUT;

    while timeout != 0 {
        if (read32(addr) & mask) != 0 {
            return true;
        }
        sdelay(1);
        timeout -= 1;
    }

    let _ = CLK_WAIT_FAIL_ADDR.compare_exchange(0, addr, Ordering::Relaxed, Ordering::Relaxed);
    false
}

pub fn sunxi_clk_get_fail_addr() -> u32 {
    CLK_WAIT_FAIL_ADDR.load(Ordering::Relaxed)
}

fn set_pll_cpux_axi() {
    let mut val: u32;

    write32(ccu::BASE + ccu::CPU_AXI_CFG_REG, (4 << 24) | (1 << 0));
    sdelay(10);

    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !(1 << 27);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 30;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);
    sdelay(5);

    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !((0x3 << 16) | (0xff << 8) | (0x3 << 0));
    val |= 41 << 8;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 29;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 31;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    if !wait_for_bit_set(ccu::BASE + ccu::PLL_CPU_CTRL_REG, 0x1 << 28) {
        return;
    }
    sdelay(20);

    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 27;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !(1 << 29);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);
    sdelay(1);

    val = read32(ccu::BASE + ccu::CPU_AXI_CFG_REG);
    val &= !(0x07 << 24 | 0x3 << 16 | 0x3 << 8 | 0xf << 0);
    val |= 0x03 << 24 | 0x1 << 8 | 0x1 << 0;
    write32(ccu::BASE + ccu::CPU_AXI_CFG_REG, val);
    sdelay(1);
}

fn set_pll_periph0() {
    let mut val: u32;

    if (read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG) & (1 << 31)) != 0 {
        return;
    }

    write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, 0x63 << 8);

    val = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
    val |= 1 << 29;
    write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, val);

    val = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
    val |= 1 << 31;
    write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, val);

    if !wait_for_bit_set(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, 0x1 << 28) {
        return;
    }
    sdelay(20);

    val = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
    val &= !(1 << 29);
    write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, val);
}

fn set_ahb() {
    write32(
        ccu::BASE + ccu::PSI_CLK_REG,
        (2 << 0) | (0 << 8) | (0x03 << 24),
    );
    sdelay(1);
}

fn set_apb() {
    write32(
        ccu::BASE + ccu::APB0_CLK_REG,
        (2 << 0) | (1 << 8) | (0x03 << 24),
    );
    sdelay(1);
}

fn set_dma() {
    write32(
        ccu::BASE + ccu::DMA_BGR_REG,
        read32(ccu::BASE + ccu::DMA_BGR_REG) | (1 << 16),
    );
    sdelay(20);
    write32(
        ccu::BASE + ccu::DMA_BGR_REG,
        read32(ccu::BASE + ccu::DMA_BGR_REG) | (1 << 0),
    );
}

fn set_mbus() {
    let mut val: u32;

    val = read32(ccu::BASE + ccu::MBUS_CLK_REG);
    val |= 0x1 << 30;
    write32(ccu::BASE + ccu::MBUS_CLK_REG, val);
    sdelay(1);

    write32(ccu::BASE + ccu::MBUS_MAT_CLK_GATING_REG, 0x00000d87);
}

fn set_module(addr: u32) {
    let mut val: u32;

    if (read32(addr) & (1 << 31)) == 0 {
        val = read32(addr);
        write32(addr, val | (1 << 31) | (1 << 30));

        val = read32(addr);
        val |= 1 << 29;
        write32(addr, val);

        if !wait_for_bit_set(addr, 0x1 << 28) {
            return;
        }
        sdelay(20);

        val = read32(addr);
        val &= !(1 << 29);
        write32(addr, val);
    }
}

pub fn init() {
    set_pll_cpux_axi();
    set_pll_periph0();
    set_ahb();
    set_apb();
    set_dma();
    set_mbus();
    set_module(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
    set_module(ccu::BASE + ccu::PLL_VIDEO0_CTRL_REG);
    set_module(ccu::BASE + ccu::PLL_VIDEO1_CTRL_REG);
    set_module(ccu::BASE + ccu::PLL_VE_CTRL);
    set_module(ccu::BASE + ccu::PLL_AUDIO0_CTRL_REG);
    set_module(ccu::BASE + ccu::PLL_AUDIO1_CTRL_REG);
}

pub fn get_peri1x_rate() -> u32 {
    let reg32 = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);

    if (reg32 & (1 << 31)) != 0 {
        let plln = ((reg32 >> 8) & 0xff) as u8 + 1u8;
        let pllm = (reg32 & 0x01) as u8 + 1u8;
        let p0 = ((reg32 >> 16) & 0x03) as u8 + 1u8;
        return (((24 * plln) / (pllm * p0)) >> 1) as u32 * 1000 * 1000;
    }
    0u32
}

pub fn sunxi_clk_dump() -> u32 {
    let reg32 = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    let p0 = (reg32 >> 16) as u8 & 0x03;
    let p1 = match p0 {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => 1,
    };
    ((((reg32 >> 8) & 0xff) + 1) * 24) / (p1 as u32)
}
