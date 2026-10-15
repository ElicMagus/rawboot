pub const BASE: u32 = 0x0200_1000;

pub const PLL_CPU_CTRL_REG: u32 = 0x000;
pub const PLL_DDR_CTRL_REG: u32 = 0x010;
pub const PLL_PERI0_CTRL_REG: u32 = 0x020;
pub const PLL_PERI1_CTRL_REG: u32 = 0x028;
pub const PLL_GPU_CTRL_REG: u32 = 0x030;
pub const PLL_VIDEO0_CTRL_REG: u32 = 0x040;
pub const PLL_VIDEO1_CTRL_REG: u32 = 0x048;
pub const PLL_VE_CTRL: u32 = 0x058;
pub const PLL_DE_CTRL: u32 = 0x060;
pub const PLL_HSIC_CTRL: u32 = 0x070;
pub const PLL_AUDIO0_CTRL_REG: u32 = 0x078;
pub const PLL_AUDIO1_CTRL_REG: u32 = 0x080;
pub const PLL_DDR_PAT0_CTRL_REG: u32 = (0x110);
pub const PLL_DDR_PAT1_CTRL_REG: u32 = (0x114);
pub const PLL_PERI0_PAT0_CTRL_REG: u32 = (0x120);
pub const PLL_PERI0_PAT1_CTRL_REG: u32 = (0x124);
pub const PLL_PERI1_PAT0_CTRL_REG: u32 = (0x128);
pub const PLL_PERI1_PAT1_CTRL_REG: u32 = (0x12c);
pub const PLL_GPU_PAT0_CTRL_REG: u32 = (0x130);
pub const PLL_GPU_PAT1_CTRL_REG: u32 = (0x134);
pub const PLL_VIDEO0_PAT0_CTRL_REG: u32 = (0x140);
pub const PLL_VIDEO0_PAT1_CTRL_REG: u32 = (0x144);
pub const PLL_VIDEO1_PAT0_CTRL_REG: u32 = (0x148);
pub const PLL_VIDEO1_PAT1_CTRL_REG: u32 = (0x14c);
pub const PLL_VE_PAT0_CTRL_REG: u32 = (0x158);
pub const PLL_VE_PAT1_CTRL_REG: u32 = (0x15c);
pub const PLL_DE_PAT0_CTRL_REG: u32 = (0x160);
pub const PLL_DE_PAT1_CTRL_REG: u32 = (0x164);
pub const PLL_HSIC_PAT0_CTRL_REG: u32 = (0x170);
pub const PLL_HSIC_PAT1_CTRL_REG: u32 = (0x174);
pub const PLL_AUDIO0_PAT0_CTRL_REG: u32 = (0x178);
pub const PLL_AUDIO0_PAT1_CTRL_REG: u32 = (0x17c);
pub const PLL_AUDIO1_PAT0_CTRL_REG: u32 = (0x180);
pub const PLL_AUDIO1_PAT1_CTRL_REG: u32 = (0x184);
pub const PLL_CPU_BIAS_REG: u32 = (0x300);
pub const PLL_DDR_BIAS_REG: u32 = (0x310);
pub const PLL_PERI0_BIAS_REG: u32 = (0x320);
pub const PLL_PERI1_BIAS_REG: u32 = (0x328);
pub const PLL_GPU_BIAS_REG: u32 = (0x330);
pub const PLL_VIDEO0_BIAS_REG: u32 = (0x340);
pub const PLL_VIDEO1_BIAS_REG: u32 = (0x348);
pub const PLL_VE_BIAS_REG: u32 = (0x358);
pub const PLL_DE_BIAS_REG: u32 = (0x360);
pub const PLL_HSIC_BIAS_REG: u32 = (0x370);
pub const PLL_AUDIO0_BIAS_REG: u32 = (0x378);
pub const PLL_AUDIO1_BIAS_REG: u32 = (0x380);
pub const PLL_CPU_TUN_REG: u32 = (0x400);
pub const CPU_AXI_CFG_REG: u32 = (0x500);
pub const CPU_GATING_REG: u32 = (0x504);
pub const PSI_CLK_REG: u32 = (0x510);
pub const AHB3_CLK_REG: u32 = (0x51c);
pub const APB0_CLK_REG: u32 = (0x520);
pub const APB1_CLK_REG: u32 = (0x524);
pub const MBUS_CLK_REG: u32 = (0x540);
pub const DMA_BGR_REG: u32 = (0x70c);
pub const DRAM_CLK_REG: u32 = (0x800);
pub const MBUS_MAT_CLK_GATING_REG: u32 = (0x804);
pub const DRAM_BGR_REG: u32 = 0x80c;
pub const USART_BGR_REG: u32 = 0x90c; //UART Bus Gating Reset Register
pub const RISCV_CLK_REG: u32 = (0xd00);
pub const RISCV_GATING_REG: u32 = (0xd04);
pub const RISCV_CFG_BGR_REG: u32 = (0xd0c);

// pub enum CcuUartReset {
//     Assert=0,
//     De_Assert=1,
// }

// pub enum CcuUartGating{
//     Mask=0,
//     Pass=1,
// }
