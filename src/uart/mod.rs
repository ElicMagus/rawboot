
use crate::ccu;
use crate::gpio;
use core::fmt;

pub const UART_BASE: u32 = 0x0250_0000;
const UART_MX: u32 = 0x400;
pub const UART_0_BASE: u32 = UART_BASE + UART_MX * 0;
// pub const UART_1_BASE: u32 = UART_BASE + UART_MX*1;
// pub const UART_2_BASE: u32 = UART_BASE + UART_MX*2;
// pub const UART_3_BASE: u32 = UART_BASE + UART_MX*3;
// pub const UART_4_BASE: u32 = UART_BASE + UART_MX*4;
// pub const UART_5_BASE: u32 = UART_BASE + UART_MX*5;

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

#[derive(Default)]
#[repr(C)]
pub struct UartRegT;

impl fmt::Write for UartRegT {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            let addr = (UART_0_BASE + 0x7c) as *mut u32;
            unsafe {
                while (core::ptr::read_volatile(addr) & (0x1 << 1)) == 0 {}
                core::ptr::write_volatile((UART_0_BASE) as *mut u32, c as u32);
                let addr = (UART_0_BASE + 0x7c) as *mut u32;
                while core::ptr::read_volatile(addr) & (0x1 << 0) == 1 {}
            }
        }

        Ok(())
    }
}
pub fn console() -> impl core::fmt::Write {
    UartRegT {}
}

pub fn init() {
    /* Config usart TXD and RXD pins */
    let gpio_e = gpio::GPIO_E as *mut gpio::GpioRegT;
    gpio::init(gpio_e, 2, gpio::PE2_Select::Uart0Tx as u8);
    gpio::init(gpio_e, 3, gpio::PE3_Select::Uart0Rx as u8);

    /* Open the clock gate for usart */
    let addr = (ccu::BASE + ccu::USART_BGR_REG) as *mut u32;
    unsafe {
        let mut val = core::ptr::read_volatile(addr);
        val |= 1 << 0;
        core::ptr::write_volatile(addr, val);
        /* Deassert USART reset */
        let mut val = core::ptr::read_volatile(addr);
        val |= 1 << (16 + 0);
        core::ptr::write_volatile(addr, val);
    }

    /* Config USART to 115200-8-1-0 */
    let addr = (UART_0_BASE + 0x04) as *mut u32;
    unsafe {
        core::ptr::write_volatile(addr, 0x0);
    }
    let addr = (UART_0_BASE + 0x08) as *mut u32;
    unsafe {
        core::ptr::write_volatile(addr, 0xf7);
    }
    let addr = (UART_0_BASE + 0x10) as *mut u32;
    unsafe {
        core::ptr::write_volatile(addr, 0x0);
    }
    let addr = (UART_0_BASE + 0x0c) as *mut u32;
    unsafe {
        let mut val = core::ptr::read_volatile(addr);
        val |= 1 << 7;
        core::ptr::write_volatile(addr, val);
    }
    let addr = (UART_0_BASE + 0x00) as *mut u32;
    unsafe {
        core::ptr::write_volatile(addr, 0xd & 0xff);
    }
    let addr = (UART_0_BASE + 0x04) as *mut u32;
    unsafe {
        core::ptr::write_volatile(addr, (0xd >> 8) & 0xff);
    }

    let addr = (UART_0_BASE + 0x0c) as *mut u32;
    unsafe {
        let mut val = core::ptr::read_volatile(addr);
        val &= !(1 << 7);
        core::ptr::write_volatile(addr, val);
        val = core::ptr::read_volatile(addr);
        val &= !(0x1f);
        val |= (0x3 << 0) | (0 << 2) | (0x0 << 3);
        core::ptr::write_volatile(addr, val);
    }
}
