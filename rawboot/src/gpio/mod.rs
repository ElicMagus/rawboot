// SPDX-License-Identifier: Unlicense

use crate::io::{read32, write32};

const GPIO_CFG0: u32 = 0x00;
const GPIO_DAT: u32 = 0x10;
const GPIO_DRV0: u32 = 0x14;
const GPIO_PUL0: u32 = 0x24;

pub const PORTB: u32 = 0;
pub const PORTC: u32 = 1;
pub const PORTD: u32 = 2;
pub const PORTE: u32 = 3;
pub const PORTF: u32 = 4;
pub const PORTG: u32 = 5;

pub const PIO_NUM_IO_BITS: u32 = 5;

pub type gpio_t = u32;

pub const fn gpio_pin(port: u32, pin: u32) -> gpio_t {
    (port << PIO_NUM_IO_BITS) | pin
}

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

pub type gpio_pull_t = u32;

pub const GPIO_PULL_UP: gpio_pull_t = 0;
pub const GPIO_PULL_DOWN: gpio_pull_t = 1;
pub const GPIO_PULL_NONE: gpio_pull_t = 2;

#[derive(Debug, Clone, Copy)]
pub struct gpio_mux_t {
    pub pin: gpio_t,
    pub mux: u32,
}

#[inline(always)]
fn port_num(pin: gpio_t) -> u32 {
    pin >> PIO_NUM_IO_BITS
}

#[inline(always)]
fn port_base_get(pin: gpio_t) -> u32 {
    match port_num(pin) {
        PORTB => 0x0200_0030,
        PORTC => 0x0200_0060,
        PORTD => 0x0200_0090,
        PORTE => 0x0200_00c0,
        PORTF => 0x0200_00f0,
        PORTG => 0x0200_0120,
        _ => 0,
    }
}

#[inline(always)]
fn pin_num(pin: gpio_t) -> u32 {
    pin & ((1 << PIO_NUM_IO_BITS) - 1)
}

pub fn sunxi_gpio_init(pin: gpio_t, cfg: u32) {
    let port_addr = port_base_get(pin);
    let pin_num = pin_num(pin);
    let addr = port_addr + GPIO_CFG0 + ((pin_num >> 3) << 2);
    let mut val = read32(addr);

    val &= !(0xf << ((pin_num & 0x7) << 2));
    if cfg != 0 {
        val |= (cfg & 0xf) << ((pin_num & 0x7) << 2);
    }
    write32(addr, val);
}

pub fn sunxi_gpio_write(pin: gpio_t, value: i32) {
    let port_addr = port_base_get(pin);
    let pin_num = pin_num(pin);
    let mut val = read32(port_addr + GPIO_DAT);

    val &= !(1 << pin_num);
    val |= ((value != 0) as u32) << pin_num;
    write32(port_addr + GPIO_DAT, val);
}

pub fn sunxi_gpio_read(pin: gpio_t) -> i32 {
    let port_addr = port_base_get(pin);
    let pin_num = pin_num(pin);
    let val = read32(port_addr + GPIO_DAT);

    ((val & (1 << pin_num)) != 0) as i32
}

pub fn sunxi_gpio_set_pull(pin: gpio_t, pull: gpio_pull_t) {
    let port_addr = port_base_get(pin);
    let pin_num = pin_num(pin);
    let v = match pull {
        GPIO_PULL_UP => 0x1,
        GPIO_PULL_DOWN => 0x2,
        GPIO_PULL_NONE => 0x0,
        _ => 0x0,
    };
    let addr = port_addr + GPIO_PUL0 + ((pin_num >> 4) << 2);
    let mut val = read32(addr);

    val &= !(v << ((pin_num & 0xf) << 1));
    val |= v << ((pin_num & 0xf) << 1);
    write32(addr, val);
}

pub fn sunxi_gpio_set_drive_lvl(pin: gpio_t, lvl: u8) {
    let port_addr = port_base_get(pin);
    let pin_num = pin_num(pin);
    let lvl = (lvl as u32) & 3;
    let addr = port_addr + GPIO_DRV0 + ((pin_num >> 4) << 2);
    let mut val = read32(addr);

    val &= !(lvl << ((pin_num & 0xf) << 1));
    val |= lvl << ((pin_num & 0xf) << 1);
    write32(addr, val);
}
