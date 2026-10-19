// SPDX-License-Identifier: Unlicense

use crate::ccu;
use crate::gpio::{
    GPIO_PERIPH_MUX6, GPIO_PULL_UP, PORTE, gpio_mux_t, gpio_pin, sunxi_gpio_init,
    sunxi_gpio_set_pull,
};
use crate::io::{read32, write32};
use core::fmt;

const UART_RBR: u32 = 0x00;
const UART_THR: u32 = 0x00;
const UART_DLL: u32 = 0x00;
const UART_FCR: u32 = 0x08;
const UART_LCR: u32 = 0x0C;
const UART_USR: u32 = 0x7C;

const UART_USR_BUSY: u32 = 1 << 0;
const UART_USR_TFNF: u32 = 1 << 1;
const UART_USR_RFNE: u32 = 1 << 3;

#[inline(always)]
fn UART_BASE(id: u8) -> u32 {
    0x0250_0000 + (0x400 * id as u32)
}

#[derive(Debug, Clone, Copy)]
pub struct sunxi_usart_t {
    pub id: u8,
    pub gpio_tx: gpio_mux_t,
    pub gpio_rx: gpio_mux_t,
}

pub const usart0_dbg: sunxi_usart_t = sunxi_usart_t {
    id: 0,
    gpio_tx: gpio_mux_t {
        pin: gpio_pin(PORTE, 2),
        mux: GPIO_PERIPH_MUX6,
    },
    gpio_rx: gpio_mux_t {
        pin: gpio_pin(PORTE, 3),
        mux: GPIO_PERIPH_MUX6,
    },
};

pub fn sunxi_usart_init(usart: &sunxi_usart_t, baudrate: u32) {
    /* Config usart TXD and RXD pins */
    sunxi_gpio_init(usart.gpio_tx.pin, usart.gpio_tx.mux);
    sunxi_gpio_init(usart.gpio_rx.pin, usart.gpio_rx.mux);
    sunxi_gpio_set_pull(usart.gpio_tx.pin, GPIO_PULL_UP);
    sunxi_gpio_set_pull(usart.gpio_rx.pin, GPIO_PULL_UP);

    /* Open the clock gate for usart */
    let addr = ccu::BASE + ccu::USART_BGR_REG;
    let mut val = read32(addr);
    val |= 1 << usart.id;
    write32(addr, val);

    /* Deassert USART reset */
    let addr = ccu::BASE + ccu::USART_BGR_REG;
    let mut val = read32(addr);
    val |= 1 << (16 + usart.id);
    write32(addr, val);

    let mut div: u32 = 1_500_000 / baudrate;
    if div == 0 {
        div = 1;
    }

    /* Configure baud rate and 8N1, enable the FIFO */
    let base = UART_BASE(usart.id);
    write32(base + UART_LCR, (1 << 7) | 3);
    write32(base + UART_DLL, div);
    write32(base + UART_LCR, 3);
    write32(base + UART_FCR, 0x0000_0001);
}

pub fn sunxi_usart_putc(usart: &sunxi_usart_t, c: u8) {
    let base = UART_BASE(usart.id);

    while (read32(base + UART_USR) & UART_USR_TFNF) == 0 {}
    write32(base + UART_THR, c as u32);
    while (read32(base + UART_USR) & UART_USR_BUSY) == 1 {}
}

pub fn sunxi_usart_put(usart: &sunxi_usart_t, data: &[u8]) {
    for &c in data {
        sunxi_usart_putc(usart, c);
    }
}

pub fn sunxi_usart_get(usart: &sunxi_usart_t, out: &mut [u8]) {
    let base = UART_BASE(usart.id);

    for c in out.iter_mut() {
        while (read32(base + UART_USR) & UART_USR_RFNE) == 0 {}
        *c = (read32(base + UART_RBR) & 0xff) as u8;
    }
}

pub struct UartConsole;

impl fmt::Write for UartConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        sunxi_usart_put(&usart0_dbg, s.as_bytes());
        Ok(())
    }
}

pub fn console() -> impl core::fmt::Write {
    UartConsole
}
