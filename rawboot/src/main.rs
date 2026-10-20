// SPDX-License-Identifier: Unlicense

#![no_std]
#![no_main]
use core::panic::PanicInfo;

use rawboot_rt::entry;
entry!(main);

use rawboot_rt::cpu::{udelay, wfe};

use crate::io::{read32, write32};
mod boot;
pub mod ccu;
pub mod clk;
pub mod dram;
mod fat;
pub mod gpio;
pub mod io;
pub mod print;
pub mod sdcard;
pub mod sdhci;
pub mod uart;

fn main() -> ! {
    // LED on port D pin 22, as upstream board_init_led()
    clk::sunxi_clk_init();
    let led = gpio::gpio_pin(gpio::PORTD, 22);
    gpio::sunxi_gpio_init(led, gpio::GPIO_OUTPUT);
    gpio::sunxi_gpio_write(led, 0);

    uart::sunxi_usart_init(&uart::usart0_dbg, 115200);
    uart::sunxi_usart_put(&uart::usart0_dbg, b"Hello\n");
    let clk_fail = clk::sunxi_clk_get_fail_addr();
    if clk_fail != 0 {
        print!("CLK: init timeout at 0x{:08x}\r\n", clk_fail);
    }
    clk::sunxi_clk_dump();
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
    let mut dram = dram::DramPara::default();
    let dram_size = dram.init_dram();

    let mut sdio = sdhci::Sdhci::default();
    sdhci::sunxi_sdhci_init(&mut sdio);
    let card = sdcard::Sdcard {
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
        card,
        hci: sdio,
        buf: [0u8; 512],
        online: false,
    };

    sdcard::sdmmc_init(&mut sdcard);

    if sdcard.online {
        let mut block = [0u8; 512];
        let read = sdcard::sdmmc_blk_read(&mut sdcard, block.as_mut_ptr(), 0, 1);
        print!(
            "SD: read {} block(s), first bytes {:02x?}\r\n",
            read,
            &block[..4]
        );

        let err = boot::boot(&mut sdcard, dram_size);
        print!("BOOT: {err}\r\n");
    }

    //мигаем
    loop {
        let status = read32(sdcard.hci.reg + sdhci::reg::RINT);
        print!("S=0x{:x}\n", status);
        gpio::sunxi_gpio_write(led, 1);
        udelay(50000);
        // write32(sdio.addr + 56, 1 << 16);
        gpio::sunxi_gpio_write(led, 0);
        udelay(50000);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
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
