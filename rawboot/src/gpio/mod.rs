// SPDX-License-Identifier: Unlicense

use crate::io::{read32, write32};

pub const GPIO_BASE: u32 = 0x0200_0000;
const MP: u32 = 0x30;
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum Port {
    B = GPIO_BASE + MP * 1,
    C = GPIO_BASE + MP * 2,
    D = GPIO_BASE + MP * 3,
    E = GPIO_BASE + MP * 4,
    F = GPIO_BASE + MP * 5,
    G = GPIO_BASE + MP * 6,
}
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum FUNC {
    INPUT = 0b0000,
    OUTPUT = 0b0001,
    MUX2 = 2,
    MUX3 = 3,
    MUX4 = 4,
    MUX5 = 5,
    MUX6 = 6,
    MUX7 = 7,
    MUX8 = 8,
    MUX14 = 14,
    DISABLED = 0xf,
}
#[derive(Default, Debug)]
#[repr(C)]
struct GpioRegT {
    cfg: u128,
    dat: u32, //0-31 pin state; 1 bit per pipn
    drv: u128,
    pull: u64, //0x0114 2 bit per pin
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Pull {
    NO_PULL = 0x0,
    UP = 0x01,
    DOWN = 0x10,
}
#[derive(Debug, Clone, Copy)]
pub struct Gpio {
    pub port: Port,
    pub pin: u8,
    pub func: FUNC,
    pub pull_mode: Pull,
}

impl Gpio {
    // #[inline(always)]
    #[unsafe(no_mangle)]
    pub fn init(&self) {
        let port = (self.port as u32) as *mut GpioRegT;
        unsafe {
            (*port).cfg &= !(0b1111 << (self.pin * 4));
            (*port).cfg |= (self.func as u128) << (self.pin * 4);
            (*port).pull &= !(0b1111 << (self.pin * 2));
            (*port).pull |= (self.pull_mode as u64) << (self.pin * 2);
        }
    }
    pub fn toggle_lvl(&self) {
        let port = (self.port as u32) as *mut GpioRegT;
        unsafe {
            (*port).dat ^= 1 << self.pin as u32;
        }
    }

    pub fn set_logic(&self, lvl: bool) {
        let port = (self.port as u32) as *mut GpioRegT;
        if lvl {
            unsafe {
                (*port).dat |= 1 << self.pin as u32;
            }
        } else {
            unsafe {
                (*port).dat &= !(1 << self.pin as u32);
            }
        }
    }
}

pub mod pd22 {
    use super::FUNC;

    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const OWA_OUT: FUNC = FUNC::MUX2;
    pub const IR_RX: FUNC = FUNC::MUX3;
    pub const UART1_RX: FUNC = FUNC::MUX4;
    pub const PWM7: FUNC = FUNC::MUX5;
    pub const PD_EINT22: FUNC = FUNC::MUX14;
    pub const DISABLE: FUNC = FUNC::DISABLED;
}

pub mod pe3 {
    use super::FUNC;

    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const NCSI0_MCLK: FUNC = FUNC::MUX2;
    pub const UART2_RX: FUNC = FUNC::MUX3;
    pub const TWI0_SDA: FUNC = FUNC::MUX4;
    pub const CLK_FANOUT1: FUNC = FUNC::MUX5;
    pub const UART0_RX: FUNC = FUNC::MUX6;
    pub const RMII_TXCK: FUNC = FUNC::MUX8;
    pub const PE_EINT3: FUNC = FUNC::MUX14;
    pub const DISABLE: FUNC = FUNC::DISABLED;
}

pub mod pe2 {
    use super::FUNC;

    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const NCSI0_PCLK: FUNC = FUNC::MUX2;
    pub const UART2_TX: FUNC = FUNC::MUX3;
    pub const TWI0_SCK: FUNC = FUNC::MUX4;
    pub const CLK_FANOUT0: FUNC = FUNC::MUX5;
    pub const UART0_TX: FUNC = FUNC::MUX6;
    pub const RMII_RXD1: FUNC = FUNC::MUX8;
    pub const PE_EINT2: FUNC = FUNC::MUX14;
    pub const DISABLE: FUNC = FUNC::DISABLED;
}

pub mod pf0 {
    use super::FUNC;
    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const DISABLE: FUNC = FUNC::DISABLED;
    pub const SDC0_D1: FUNC = FUNC::MUX2;
    pub const JTAG_MS: FUNC = FUNC::MUX3;
    pub const I2S2_DOUT1: FUNC = FUNC::MUX5;
    pub const I2S2_DIN0: FUNC = FUNC::MUX6;
    pub const PF_EINT0: FUNC = FUNC::MUX14;
}

pub mod pf1 {
    use super::FUNC;
    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const DISABLE: FUNC = FUNC::DISABLED;
    pub const SDC0_D0: FUNC = FUNC::MUX2;
    pub const JTAG_DI: FUNC = FUNC::MUX3;
    pub const I2S2_DOUT: FUNC = FUNC::MUX5;
    pub const I2S2_DIN1: FUNC = FUNC::MUX6;
    pub const PF_EINT1: FUNC = FUNC::MUX14;
}
pub mod pf2 {
    use super::FUNC;
    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const DISABLE: FUNC = FUNC::DISABLED;
    pub const SDC0_CLK: FUNC = FUNC::MUX2;
    pub const UART0_TX: FUNC = FUNC::MUX3;
    pub const TWI0_SCK: FUNC = FUNC::MUX4;
    pub const LEDC_DO: FUNC = FUNC::MUX5;
    pub const OWA_IN: FUNC = FUNC::MUX6;
    pub const PF_EINT2: FUNC = FUNC::MUX14;
}
pub mod pf3 {
    use super::FUNC;
    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const DISABLE: FUNC = FUNC::DISABLED;
    pub const SDC0_CMD: FUNC = FUNC::MUX2;
    pub const JTAG_DO: FUNC = FUNC::MUX3;
    pub const I2S2_BCLK: FUNC = FUNC::MUX5;
    pub const PF_EINT3: FUNC = FUNC::MUX14;
}
pub mod pf4 {
    use super::FUNC;
    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const DISABLE: FUNC = FUNC::DISABLED;
    pub const SDC0_D3: FUNC = FUNC::MUX2;
    pub const UART0_RX: FUNC = FUNC::MUX3;
    pub const TWI0_SDA: FUNC = FUNC::MUX4;
    pub const PWM6: FUNC = FUNC::MUX5;
    pub const IR_TX: FUNC = FUNC::MUX6;
    pub const PF_EINT4: FUNC = FUNC::MUX14;
}
pub mod pf5 {
    use super::FUNC;
    pub const INPUT: FUNC = FUNC::INPUT;
    pub const OUTPUT: FUNC = FUNC::OUTPUT;
    pub const DISABLE: FUNC = FUNC::DISABLED;
    pub const SDC0_D2: FUNC = FUNC::MUX2;
    pub const JTAG_CK: FUNC = FUNC::MUX3;
    pub const I2S2_LRCK: FUNC = FUNC::MUX5;
    pub const PF_EINT5: FUNC = FUNC::MUX14;
}
