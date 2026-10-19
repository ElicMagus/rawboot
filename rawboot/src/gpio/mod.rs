// SPDX-License-Identifier: Unlicense

use crate::io::{read32, write32};

const GPIO_CFG0: u32 = 0x00;
const GPIO_DAT: u32 = 0x10;
const GPIO_DRV0: u32 = 0x14;
const GPIO_PUL0: u32 = 0x24;

const GPIO_PORTB_BASE: u32 = 0x0200_0030;
const GPIO_PORTC_BASE: u32 = 0x0200_0060;
const GPIO_PORTD_BASE: u32 = 0x0200_0090;
const GPIO_PORTE_BASE: u32 = 0x0200_00c0;
const GPIO_PORTF_BASE: u32 = 0x0200_00f0;
const GPIO_PORTG_BASE: u32 = 0x0200_0120;

const GPIO_CFG_PINS_PER_REG: u32 = 8;
const GPIO_CFG_PIN_SHIFT: u32 = 3;
const GPIO_CFG_BITS_PER_PIN: u32 = 4;
const GPIO_CFG_FIELD_SHIFT: u32 = 2;
const GPIO_CFG_MASK: u32 = (1 << GPIO_CFG_BITS_PER_PIN) - 1;
const GPIO_PIN_NUM_MASK: u32 = GPIO_CFG_PINS_PER_REG - 1;

const GPIO_PULL_PINS_PER_REG: u32 = 16;
const GPIO_PULL_PIN_SHIFT: u32 = 4;
const GPIO_PULL_BITS_PER_PIN: u32 = 2;
const GPIO_PULL_FIELD_SHIFT: u32 = 1;
const GPIO_PULL_PIN_MASK: u32 = GPIO_PULL_PINS_PER_REG - 1;

const GPIO_DRV_PIN_MASK: u32 = GPIO_PULL_PINS_PER_REG - 1;
const GPIO_DRV_BITS_MAX: u32 = (1 << GPIO_PULL_BITS_PER_PIN) - 1;

pub const PIO_NUM_IO_BITS: u32 = 5;

pub type GpioPin = u32;

pub const fn gpio_pin(port: u32, pin: u32) -> GpioPin {
    (port << PIO_NUM_IO_BITS) | pin
}

pub const PORTB: u32 = 0;
pub const PORTC: u32 = 1;
pub const PORTD: u32 = 2;
pub const PORTE: u32 = 3;
pub const PORTF: u32 = 4;
pub const PORTG: u32 = 5;

pub const GPIO_INPUT: u32 = 0;
pub const GPIO_OUTPUT: u32 = 1;
pub const GPIO_PERIPH_MUX2: u32 = 2;
pub const GPIO_PERIPH_MUX3: u32 = 3;
pub const GPIO_PERIPH_MUX4: u32 = 4;
pub const GPIO_PERIPH_MUX5: u32 = 5;
pub const GPIO_PERIPH_MUX6: u32 = 6;
pub const GPIO_PERIPH_MUX7: u32 = 7;
pub const GPIO_PERIPH_MUX8: u32 = 8;
pub const GPIO_PERIPH_MUX14: u32 = 14;
pub const GPIO_DISABLED: u32 = 0xf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioPull {
    Up,
    Down,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct GpioMux {
    pub pin: GpioPin,
    pub mux: u32,
}

#[inline(always)]
fn port_base_get(pin: GpioPin) -> u32 {
    match pin >> PIO_NUM_IO_BITS {
        PORTB => GPIO_PORTB_BASE,
        PORTC => GPIO_PORTC_BASE,
        PORTD => GPIO_PORTD_BASE,
        PORTE => GPIO_PORTE_BASE,
        PORTF => GPIO_PORTF_BASE,
        PORTG => GPIO_PORTG_BASE,
        _ => 0,
    }
}

#[inline(always)]
fn pin_num(pin: GpioPin) -> u32 {
    pin & ((1 << PIO_NUM_IO_BITS) - 1)
}

pub fn sunxi_gpio_init(pin: GpioPin, cfg: u32) {
    let port_addr = port_base_get(pin);
    let num = pin_num(pin);
    let addr = port_addr + GPIO_CFG0 + ((num >> GPIO_CFG_PIN_SHIFT) << GPIO_CFG_FIELD_SHIFT);
    let mut val = read32(addr);

    val &= !(GPIO_CFG_MASK << ((num & GPIO_PIN_NUM_MASK) << GPIO_CFG_FIELD_SHIFT));
    if cfg != 0 {
        val |= (cfg & GPIO_CFG_MASK) << ((num & GPIO_PIN_NUM_MASK) << GPIO_CFG_FIELD_SHIFT);
    }
    write32(addr, val);
}

pub fn sunxi_gpio_write(pin: GpioPin, value: i32) {
    let port_addr = port_base_get(pin);
    let num = pin_num(pin);
    let mut val = read32(port_addr + GPIO_DAT);

    val &= !(1 << num);
    val |= ((value != 0) as u32) << num;
    write32(port_addr + GPIO_DAT, val);
}

pub fn sunxi_gpio_read(pin: GpioPin) -> i32 {
    let port_addr = port_base_get(pin);
    let num = pin_num(pin);
    let val = read32(port_addr + GPIO_DAT);

    ((val & (1 << num)) != 0) as i32
}

pub fn sunxi_gpio_set_pull(pin: GpioPin, pull: GpioPull) {
    let port_addr = port_base_get(pin);
    let num = pin_num(pin);
    let v = match pull {
        GpioPull::Up => 0x1,
        GpioPull::Down => 0x2,
        GpioPull::None => 0x0,
    };
    let addr = port_addr + GPIO_PUL0 + ((num >> GPIO_PULL_PIN_SHIFT) << GPIO_PULL_FIELD_SHIFT);
    let mut val = read32(addr);

    val &= !(v << ((num & GPIO_PULL_PIN_MASK) << GPIO_PULL_FIELD_SHIFT));
    val |= v << ((num & GPIO_PULL_PIN_MASK) << GPIO_PULL_FIELD_SHIFT);
    write32(addr, val);
}

pub fn sunxi_gpio_set_drive_lvl(pin: GpioPin, lvl: u8) {
    let port_addr = port_base_get(pin);
    let num = pin_num(pin);
    let lvl = (lvl as u32) & GPIO_DRV_BITS_MAX;
    let addr = port_addr + GPIO_DRV0 + ((num >> GPIO_PULL_PIN_SHIFT) << GPIO_PULL_FIELD_SHIFT);
    let mut val = read32(addr);

    val &= !(lvl << ((num & GPIO_DRV_PIN_MASK) << GPIO_PULL_FIELD_SHIFT));
    val |= lvl << ((num & GPIO_DRV_PIN_MASK) << GPIO_PULL_FIELD_SHIFT);
    write32(addr, val);
}
