use crate::{
    ccu,
    io::{read32, write32,sdelay},    
};



fn set_pll_cpux_axi() {
    let mut val:u32;
	
    /* Select cpux clock src to osc24m, axi divide ratio is 3, system apb clk ratio is 4 */
	val = (0 << 24) | (3 << 8) | (1 << 0);
	
    write32(
        ccu::BASE + ccu::CPU_AXI_CFG_REG,
        val
    );
    sdelay(1);

    // /* Disable pll gating */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
	// print!("v={:#?}\r\n",val);
    val &= !(1 << 27);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);	

    // /* Enable pll ldo */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 30;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);
    sdelay(5);

    // /* Set default clk to 1008mhz */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !((0x3 << 16) | (0xff << 8) | (0x3 << 0));
    val |= 41 << 8;
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
    while (!(read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG) & (0x1 << 28)))==0{}
    sdelay(20);

    // /* Enable pll gating */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val |= 1 << 27;
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);

    // /* Lock disable */
    val = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
    val &= !(1 << 29);
    write32(ccu::BASE + ccu::PLL_CPU_CTRL_REG, val);
    sdelay(1);

    // /* set and change cpu clk src to PLL_CPUX, PLL_CPUX:AXI0 = 1008M:504M */
    val = read32(ccu::BASE + ccu::CPU_AXI_CFG_REG);
    val &= !(0x07 << 24 | 0x3 << 16 | 0x3 << 8 | 0xf << 0);
    val |= 0x03 << 24 | 0x0 << 16 | 0x0 << 8 | 0x1 << 0;
    write32(ccu::BASE + ccu::CPU_AXI_CFG_REG, val);
    sdelay(1);
}

fn set_pll_periph0()
{
	let mut val:u32;
    let addr:u32;

	/* Periph0 has been enabled */
    addr = ccu::BASE + ccu::PLL_PERI0_CTRL_REG;
    val = read32(addr) & (1 << 31);
	if val !=0 {
        return;
    }		

	/* Change psi src to osc24m */
	val = read32(ccu::BASE + ccu::PSI_CLK_REG);
	val &= !(0x3 << 24);
	write32(val, ccu::BASE + ccu::PSI_CLK_REG);

	/* Set default val */
	write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, 0x63 << 8);

	/* Lock enable */
	val = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
	val |= 1 << 29;
	write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, val);

	/* Enabe pll 600m(1x) 1200m(2x) */
	val = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
	val |= 1 << 31;
	write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, val);

	/* Wait pll stable */
    loop{
        if read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG) & (0x1 << 28) !=0 {
            break;
        };
    }
	
	sdelay(20);

	/* Lock disable */
	val = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
	val &= !(1 << 29);
	write32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG, val);
}

fn set_ahb()
{
	write32(ccu::BASE + ccu::PSI_CLK_REG, (2 << 0) | (0 << 8));
	write32(ccu::BASE + ccu::PSI_CLK_REG, read32(ccu::BASE + ccu::PSI_CLK_REG) | (0x03 << 24));
	sdelay(1);
}

fn set_apb()
{
	write32(ccu::BASE + ccu::APB0_CLK_REG, (2 << 0) | (1 << 8));
	write32(ccu::BASE + ccu::APB0_CLK_REG, (0x03 << 24) | read32(ccu::BASE + ccu::APB0_CLK_REG));
	sdelay(1);
}

fn set_dma()
{
	/* Dma reset */
	write32(ccu::BASE + ccu::DMA_BGR_REG, read32(ccu::BASE + ccu::DMA_BGR_REG) | (1 << 16));
	sdelay(20);
	/* Enable gating clock for dma */
	write32(ccu::BASE + ccu::DMA_BGR_REG, read32(ccu::BASE + ccu::DMA_BGR_REG) | (1 << 0));
}

fn set_mbus()
{
	let mut val:u32;

	/* Reset mbus domain */
	val = read32(ccu::BASE + ccu::MBUS_CLK_REG);
	val |= 0x1 << 30;
	write32(ccu::BASE + ccu::MBUS_CLK_REG, val);
	sdelay(1);

	/* Enable mbus master clock gating */
	write32(ccu::BASE + ccu::MBUS_MAT_CLK_GATING_REG, 0x00000d87);
}

fn set_module(addr:u32)
{    
	let mut val:u32;
   
    val=!(read32(addr) & (1 << 31));
	if val!=0
	{
		val = read32(addr);
		write32(addr, val | (1 << 31) | (1 << 30));

	// 	/* Lock enable */
		val = read32(addr);
		val |= 1 << 29;
		write32(addr, val);

		/* Wait pll stable */
        while(!(read32(addr) & (0x1 << 28)))==0{};
        sdelay(20);

	// 	/* Lock disable */
		val = read32(addr);
		val &= !(1 << 29);
		write32(addr, val);
	}
}

pub fn init()
{
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

pub fn sunxi_clk_get_peri1x_rate()->u32
{
    let reg32:u32;
    let plln:u8;
    let pllm:u8;
    let p0:u8;

	/* PLL PERIx */
	reg32 = read32(ccu::BASE + ccu::PLL_PERI0_CTRL_REG);
	if (reg32 & (1 << 31))!=0 {
	    plln = ((reg32 >> 8) & 0xff) as u8 + 1u8;
	    pllm = (reg32 & 0x01)as u8 + 1u8;
	    p0 = ((reg32 >> 16) & 0x03) as u8 + 1u8;
	    return (((24 * plln)/(pllm * p0)) >> 1) as u32 * 1000 * 1000;
	}
	0u32
}

pub fn sunxi_clk_dump()->u32
{
    let reg32:u32;
//    uint32_t cpu_clk, plln, pllm;
let p0:u8;
 let p1:u8;


	/* PLL CPU */
	// reg32 = read32(ccu::BASE + ccu::CPU_AXI_CFG_REG);
	reg32 = read32(ccu::BASE + ccu::PLL_CPU_CTRL_REG);
//	if (reg32 & (1 << 31)) {
	    p0 = (reg32 >> 16) as u8 & 0x03;
	    if p0 == 0 {
		p1 = 1;
	    } else if p0 == 1 {
		p1 = 2;
	    } else if p0 == 2 {
		p1 = 4;
	    } else {
		p1 = 1;
	    }
let pr:u32 = ((((reg32 >> 8) & 0xff) + 1) * 24)/( p1 as u32 );
		pr
	    
}