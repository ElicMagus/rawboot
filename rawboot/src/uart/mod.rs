// SPDX-License-Identifier: Unlicense

use crate::ccu;
use crate::gpio::{
    GPIO_PERIPH_MUX6, GpioMux, GpioPull, PORTE, gpio_pin, sunxi_gpio_init, sunxi_gpio_set_pull,
};
use crate::io::{read32, write32};
use core::fmt;

const UART0_BASE: u32 = 0x0250_0000;
const UART_MX: u32 = 0x400;

const UART_RBR: u32 = 0x00;
const UART_THR: u32 = 0x00;
const UART_DLL: u32 = 0x00;
const UART_FCR: u32 = 0x08;
const UART_LCR: u32 = 0x0C;
const UART_USR: u32 = 0x7C;

const UART_USR_BUSY: u32 = 1 << 0;
const UART_USR_TFNF: u32 = 1 << 1;
const UART_USR_RFNE: u32 = 1 << 3;

const UART_LCR_DLAB: u32 = 1 << 7;
const UART_LCR_8N1: u32 = 3;
const UART_FCR_FIFO_ENABLE: u32 = 0x1;
const UART_BYTE_MASK: u32 = 0xff;

const USART_BGR_GATE_SHIFT: u8 = 0;
const USART_BGR_RESET_SHIFT: u8 = 16;

const UART_CLK_HZ: u32 = 1_500_000;

#[inline(always)]
fn uart_base(id: u8) -> u32 {
    UART0_BASE + UART_MX * id as u32
}

#[derive(Debug, Clone, Copy)]
pub struct SunxiUsart {
    pub id: u8,
    pub gpio_tx: GpioMux,
    pub gpio_rx: GpioMux,
}

pub const usart0_dbg: SunxiUsart = SunxiUsart {
    id: 0,
    gpio_tx: GpioMux {
        pin: gpio_pin(PORTE, 2),
        mux: GPIO_PERIPH_MUX6,
    },
    gpio_rx: GpioMux {
        pin: gpio_pin(PORTE, 3),
        mux: GPIO_PERIPH_MUX6,
    },
};

pub fn sunxi_usart_init(usart: &SunxiUsart, baudrate: u32) {
    // Route the TX and RX pads to the USART function and pull them up
    sunxi_gpio_init(usart.gpio_tx.pin, usart.gpio_tx.mux);
    sunxi_gpio_init(usart.gpio_rx.pin, usart.gpio_rx.mux);
    sunxi_gpio_set_pull(usart.gpio_tx.pin, GpioPull::Up);
    sunxi_gpio_set_pull(usart.gpio_rx.pin, GpioPull::Up);

    // Ungate the USART clock
    let addr = ccu::BASE + ccu::USART_BGR_REG;
    let mut val = read32(addr);
    val |= 1 << (USART_BGR_GATE_SHIFT + usart.id);
    write32(addr, val);

    // Release the USART reset
    let addr = ccu::BASE + ccu::USART_BGR_REG;
    let mut val = read32(addr);
    val |= 1 << (USART_BGR_RESET_SHIFT + usart.id);
    write32(addr, val);

    let mut div: u32 = UART_CLK_HZ / baudrate;
    if div == 0 {
        div = 1;
    }

    // 8N1 at the requested baud rate, with the FIFO enabled
    let base = uart_base(usart.id);
    write32(base + UART_LCR, UART_LCR_DLAB | UART_LCR_8N1);
    write32(base + UART_DLL, div);
    write32(base + UART_LCR, UART_LCR_8N1);
    write32(base + UART_FCR, UART_FCR_FIFO_ENABLE);
}

pub fn sunxi_usart_putc(usart: &SunxiUsart, c: u8) {
    let base = uart_base(usart.id);

    while (read32(base + UART_USR) & UART_USR_TFNF) == 0 {}
    write32(base + UART_THR, c as u32);
    while (read32(base + UART_USR) & UART_USR_BUSY) == 1 {}
}

pub fn sunxi_usart_put(usart: &SunxiUsart, data: &[u8]) {
    for &c in data {
        sunxi_usart_putc(usart, c);
    }
}

pub fn sunxi_usart_get(usart: &SunxiUsart, out: &mut [u8]) {
    let base = uart_base(usart.id);

    for c in out.iter_mut() {
        while (read32(base + UART_USR) & UART_USR_RFNE) == 0 {}
        *c = (read32(base + UART_RBR) & UART_BYTE_MASK) as u8;
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
