use crate::{
    ccu,
    io::{read32, write32},
    print,
};

pub fn set_pll_cpux_axi() {
    let mut val;

    /* Select cpux clock src to osc24m, axi divide ratio is 3, system apb clk ratio is 4 */
    write32(
        ccu::BASE + ccu::CPU_AXI_CFG_REG,
        (0 << 24) | (3 << 8) | (1 << 0),
    );
    // sdelay(1);

    /* Disable pll gating */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !(1 << 27);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    /* Enable pll ldo */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= (1 << 30);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);
    // sdelay(5);

    /* Set default clk to 1008mhz */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !((0x3 << 16) | (0xff << 8) | (0x3 << 0));
    val |= (41 << 8);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    /* Lock enable */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 29;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    /* Enable pll */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 31;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    /* Wait pll stable */
    //while(!(read32(T113_ccu::BASE + ccu::PLL_CPU_CTRL_REG) & (0x1 << 28))); //Си
    loop {
        let v = !(read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG) & (0x1 << 28));
        if v == 0 {
            print!("Pll is Stable\n");
            break;
        }
    }
    // sdelay(20);

    /* Enable pll gating */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= (1 << 27);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    /* Lock disable */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !(1 << 29);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);
    // sdelay(1);

    /* set and change cpu clk src to PLL_CPUX, PLL_CPUX:AXI0 = 1008M:504M */
    val = read32(ccu::BASE + ccu::CPU_AXI_CFG_REG);
    val &= !(0x07 << 24 | 0x3 << 16 | 0x3 << 8 | 0xf << 0);
    val |= (0x03 << 24 | 0x0 << 16 | 0x0 << 8 | 0x1 << 0);
    write32(ccu::BASE + ccu::CPU_AXI_CFG_REG, val);
    // sdelay(1);
}
