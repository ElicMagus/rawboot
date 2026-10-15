use crate::ccu;
use crate::gpio;
use crate::io::{read32, write32};
use core::fmt;

const UART_BASE: u32 = 0x0250_0000;
const UART_MX: u32 = 0x400;
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum Uart {
    UART0 = UART_BASE,
    UART1 = UART_BASE + UART_MX * 1,
    UART2 = UART_BASE + UART_MX * 2,
    UART3 = UART_BASE + UART_MX * 3,
    UART4 = UART_BASE + UART_MX * 4,
    UART5 = UART_BASE + UART_MX * 5,
}

// pub const UART_RBR:u32= 0x0000;             //UART Receive Buffer Register
// pub const UART_THR:u32= 0x0000;             //UART Transmit Holding Register
// pub const UART_DLL:u32= 0x0000;             //UART Divisor Latch Low Register
// pub const UART_DLH:u32= 0x0004;             //UART Divisor Latch High Register
// pub const UART_IER:u32= 0x0004;             //UART Interrupt Enable Register
// pub const UART_IIR:u32= 0x0008;             //UART Interrupt Identity Register
// pub const UART_FCR:u32= 0x0008;             //UART FIFO Control Register
// pub const UART_LCR:u32= 0x000C;             //UART Line Control Register
// pub const UART_MCR:u32= 0x0010;             //UART Modem Control Register
// pub const UART_LSR:u32= 0x0014;             //UART Line Status Register
// pub const UART_MSR:u32= 0x0018;             //UART Modem Status Register
// pub const UART_SCH:u32= 0x001C;             //UART Scratch Register
// pub const UART_USR:u32= 0x007C;             //UART Status Register
// pub const UART_TFL:u32= 0x0080;             //UART Transmit FIFO Level Register
// pub const UART_RFL:u32= 0x0084;             //UART Receive FIFO Level Register
// pub const UART_HSK:u32= 0x0088;             //UART DMA Handshake Configuration Register
// pub const UART_DMA_REQ_EN:u32= 0x008C;      //UART DMA Request Enable Register
// pub const UART_HALT:u32= 0x00A4;            //UART Halt TX Register
// pub const UART_DBG_DLL:u32=0x00B0;          //UART Debug DLL Register
// pub const UART_DBG_DLH:u32=0x00B4;          //UART Debug DLH Register
// pub const UART_A_FCC:u32=0x00F0;            //UART FIFO Clock Control Register
// pub const UART_A_RXDMA_CTRL:u32=0x0100;     //UART RXDMA Control Register
// pub const UART_A_RXDMA_STR:u32=0x0104;      //UART RXDMA Start Register
// pub const UART_A_RXDMA_STA:u32=0x0108;      //UART RXDMA Status Register
// pub const UART_A_RXDMA_LMT:u32=0x010C;      //UART RXDMA Limit Register
// pub const UART_A_RXDMA_SADDRL:u32=0x0110;   //UART RXDMA Buffer Start Address Low Register
// pub const UART_A_RXDMA_SADDRH:u32=0x0114;   //UART RXDMA Buffer Start Address High Register
// pub const UART_A_RXDMA_BL:u32=0x0118;       //UART RXDMA Buffer Length Register
// pub const UART_A_RXDMA_IE:u32=0x0120;       //UART RXDMA Interrupt Enable Register
// pub const UART_A_RXDMA_IS:u32=0x0124;       //UART RXDMA Interrupt Status Register
// pub const UART_A_RXDMA_WADDRL:u32=0x0128;   //UART RXDMA Write Address Low Register
// pub const UART_A_RXDMA_WADDRH:u32=0x012C;   //UART RXDMA Write Address high Register
// pub const UART_A_RXDMA_RADDRL:u32=0x0130;   //UART RXDMA Read Address Low Register
// pub const UART_A_RXDMA_RADDRH:u32=0x0134;   //UART RXDMA Read Address high Register
// pub const UART_A_RXDMA_DCNT:u32=0x0138;     //UART RADMA Data Count Register

#[repr(C)]
pub struct UartRegT {}

pub struct UartHandler {
    pub base: Uart,
    pub rx: gpio::Gpio,
    pub tx: gpio::Gpio,
}

impl fmt::Write for UartRegT {
    #[inline(always)]
    fn write_str(self: &mut UartRegT, s: &str) -> fmt::Result {
        for c in s.chars() {
            let addr = Uart::UART0 as u32 + 0x7c;
            while (read32(addr) & (0x1 << 1)) == 0 {}
            write32(Uart::UART0 as u32, c as u32);
            let addr = Uart::UART0 as u32 + 0x7c;
            while read32(addr) & (0x1 << 0) == 1 {}
        }

        Ok(())
    }
}

pub fn console() -> impl core::fmt::Write {
    UartRegT {}
}

impl Default for UartHandler {
    fn default() -> Self {
        UartHandler {
            base: Uart::UART0,
            tx: gpio::Gpio {
                port: gpio::Port::E,
                pin: 2,
                func: gpio::pe2::UART0_TX,
                pull_mode: gpio::Pull::UP,
            },
            rx: gpio::Gpio {
                port: gpio::Port::E,
                pin: 2,
                func: gpio::pe3::UART0_RX,
                pull_mode: gpio::Pull::UP,
            },
        }
    }
}

impl UartHandler {
    // pub fn new(&self)->UartRegT{
    // UartRegT{ uart: todo!(), gpio_rx: todo!(), gpio_tx: todo!() }
    // }
    pub fn putc(&self, c: char) {
        while (read32(self.base as u32 + 0x7c) & (0x1 << 1)) == 0 {}
        write32(self.base as u32, c as u32);
        while read32(self.base as u32 + 0x7c) & (0x1 << 0) == 1 {}
    }

    pub fn init(&self) {
        /* Config usart TXD and RXD pins */
        self.rx.init();
        self.tx.init();
        /* Open the clock gate for usart */
        let addr = ccu::BASE + ccu::USART_BGR_REG;
        let mut val = read32(addr);
        val |= 1 << 0;
        write32(addr, val);
        /* Deassert USART reset */
        let mut val = read32(addr);
        val |= 1 << (16 + 0);
        write32(addr, val);

        /* Config USART to 115200-8-1-0 */
        let addr = self.base as u32 + 0x04;
        write32(addr, 0x0);
        let addr = self.base as u32 + 0x08;
        write32(addr, 0xf7);
        let addr = self.base as u32 + 0x10;
        write32(addr, 0x0);
        let addr = self.base as u32 + 0x0c;
        let mut val = read32(addr);
        val |= 1 << 7;
        write32(addr, val);
        let divisor: u32 = 0xd;
        let addr = self.base as u32 + 0x00;
        write32(addr, divisor & 0xff);
        let addr = self.base as u32 + 0x04;
        write32(addr, (divisor >> 8) & 0xff);
        let addr = self.base as u32 + 0x0c;
        let mut val = read32(addr);
        val &= !(1 << 7);
        write32(addr, val);
        val = read32(addr);
        val &= !(0x1f);
        val |= (0x3 << 0) | (0 << 2) | (0x0 << 3);
        write32(addr, val);
    }
}
