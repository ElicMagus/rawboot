// SPDX-License-Identifier: Unlicense

#![no_std]
#![no_main]
#![allow(warnings)]
use core::arch::asm;
use core::panic::PanicInfo;

use rawboot_rt::entry;
entry!(main);

use rawboot_rt::cpu::{get_sp, get_spl_size, set_sp, udelay, wfe};

use crate::gpio::{Port, Pull, pd22, pe2, pe3};
use crate::io::{read32, write32};
pub mod ccu;
pub mod clk;
pub mod dram;
pub mod gpio;
pub mod io;
pub mod print;
pub mod sdcard;
pub mod sdhci;
pub mod uart;

fn main() -> ! {
    // //инициализация порта D ноги 22
    clk::init();
    let led = gpio::Gpio {
        port: Port::D,
        pin: 22,
        func: pd22::OUTPUT,
        pull_mode: Pull::UP,
    };
    led.init();

    let uart = uart::UartHandler::default();
    uart.init();

    let s = "Hello\n";
    for c in s.chars() {
        uart.putc(c);
    }
    let pr = clk::sunxi_clk_dump();
    print!("CLK={:?} MHz\r\n", pr);
    /* ?? */
    let mut reg32 = read32(0x070901f4);
    if (reg32 & 0x1f) != 0 {
        reg32 = (reg32 & 0xffffff8f) | 0x40;
        reg32 = (reg32 & 0xffffff8f) | 0xc0;
        reg32 = (reg32 & 0xffffff8e) | 0xc0;
        write32(0x070901f4, reg32);
        // print!("fix vccio detect value:0x{:X?}\r\n", reg32);
    }

    let addr = 0x0200180Cu32;
    let val = (1 << 16) | (1 << 0);
    write32(addr, val);
    let dram = dram::DramPara_t::default();
    dram.init();

    let mut sdio = sdhci::sdhci_t::default();
    sdio.init();
    let mut card = sdcard::Sdcard {
        version: 0,
        ocr: 0,
        rca: 0,
        cid: [0u32; 4],
        csd: [0u32; 4],
        extcsd: [0u8; 512],
        high_capacity: 0,
        tran_speed: 0,
        dsr_imp: 0,
        read_bl_len: 0,
        write_bl_len: 0,
        capacity: 0,
    };
    let mut sdcard = sdcard::Sdcard_data {
        card: card,
        hci: sdio,
        buf: [0u8; 512],
        online: false,
    };

    sdcard.init(sdio);

    //мигаем
    loop {
        let status = read32(sdio.addr + 56); //tmp
        print!("S=0x{:x}\n", status);
        led.set_logic(true);
        udelay(50000);
        // write32(sdio.addr + 56, 1 << 16);
        led.set_logic(false);
        udelay(50000);
    }
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // let (location, line, column) = match info.location() {
    //     Some(loc) => (loc.file(), loc.line(), loc.column()),
    //     _ => ("???", 0, 0),
    // };
    // print!(
    //     "Kernel panic!\n\n\
    //     Panic location:\n      File '{}', line {}, column {}\n\n\
    // ",
    //     location, line, column,
    // );
    wfe();
    loop {}
}
