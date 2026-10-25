// SPDX-License-Identifier: Unlicense

use crate::console::info;

use core::sync::atomic::{AtomicU32, Ordering};

use sunxi_pl::io::{read32, sdelay, write32};

use crate::ccu;

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
    val &= !((0x3 << 16) | (0xff << 8) | 0x3);
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
    val &= !(0x07 << 24 | 0x3 << 16 | 0x3 << 8 | 0xf);
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
    write32(ccu::BASE + ccu::PSI_CLK_REG, 2 | (0x03 << 24));
    sdelay(1);
}

fn set_apb() {
    write32(ccu::BASE + ccu::APB0_CLK_REG, 2 | (1 << 8) | (0x03 << 24));
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

pub fn sunxi_clk_init() {
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

const fn pll_peri1x_rate(reg32: u32) -> u32 {
    if reg32 & (1 << 31) == 0 {
        return 0;
    }

    let plln = ((reg32 >> 8) & 0xff) + 1;
    let pllm = (reg32 & 0x01) + 1;
    let p0 = ((reg32 >> 16) & 0x03) + 1;
    (((24 * plln) / (pllm * p0)) >> 1) * 1000 * 1000
}

pub fn sunxi_clk_get_peri1x_rate() -> u32 {
    pll_peri1x_rate(read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG))
}

const _: () = {
    assert!(pll_peri1x_rate(0) == 0);
    assert!(pll_peri1x_rate((1u32 << 31) | (49 << 8)) == 600_000_000);
    assert!(pll_peri1x_rate((1u32 << 31) | (49 << 8) | (1 << 16)) == 300_000_000);
};

pub fn sunxi_clk_dump() {
    let reg32 = read32(ccu::BASE + ccu::CPU_AXI_CFG_REG);
    let cpu_clk_src = (reg32 >> 24) & 0x7;
    let clock_str = match cpu_clk_src {
        0x0 => "OSC24M",
        0x1 => "CLK32",
        0x2 => "CLK16M_RC",
        0x3 => "PLL_CPU",
        0x4 => "PLL_PERI(1X)",
        0x5 => "PLL_PERI(2X)",
        0x6 => "PLL_PERI(800M)",
        _ => "ERROR",
    };

    let reg32 = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    let p0 = (reg32 >> 16) & 0x03;
    let p1 = match p0 {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => 1,
    };
    info!(
        "CLK: CPU PLL={} FREQ={}MHz\r\n",
        clock_str,
        (((reg32 >> 8) & 0xff) + 1) * 24 / p1
    );

    let reg32 = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
    if (reg32 & (1 << 31)) != 0 {
        let plln = ((reg32 >> 8) & 0xff) + 1;
        let pllm = (reg32 & 0x01) + 1;
        let p0 = ((reg32 >> 16) & 0x03) + 1;
        let p1 = ((reg32 >> 20) & 0x03) + 1;
        info!(
            "CLK: PLL_peri (2X)={}MHz, (1X)={}MHz, (800M)={}MHz\r\n",
            (24 * plln) / (pllm * p0),
            ((24 * plln) / (pllm * p0)) >> 1,
            (24 * plln) / (pllm * p1)
        );
    } else {
        info!("CLK: PLL_peri disabled\r\n");
    }

    let reg32 = read32(ccu::BASE + ccu::PLL_DDR_CTRL_REG);
    if (reg32 & (1 << 31)) != 0 {
        let plln = ((reg32 >> 8) & 0xff) + 1;
        let p1 = ((reg32 >> 1) & 0x1) + 1;
        let p0 = (reg32 & 0x01) + 1;
        info!("CLK: PLL_ddr={}MHz\r\n", (24 * plln) / (p0 * p1));
    } else {
        info!("CLK: PLL_ddr disabled\r\n");
    }
}
