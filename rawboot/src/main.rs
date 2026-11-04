// SPDX-License-Identifier: Unlicense

#![no_std]
#![no_main]
use core::panic::PanicInfo;

use rawboot_rt::entry;
entry!(main);

// The timer comes from the runtime, which re-exports it from the platform
// layer; `wfe` the runtime no longer carries (ADR-0010).
use rawboot_rt::cpu::udelay;
use sunxi_pl::cpu::wfe;

use crate::io::read32;
use sunxi_drivers::{clk, dram, sdcard, sdhci, uart};

mod boot;
pub mod print;

// The shared platform layer, re-exported under the loader's module names so the
// call sites stay `crate::gpio::…` and `crate::io::…` (ADR-0010).
pub use sunxi_pl::{gpio, io};

fn main() -> ! {
    // LED on port D pin 22, as upstream board_init_led()
    clk::sunxi_clk_init();
    let led = gpio::gpio_pin(gpio::PORTD, 22);
    gpio::sunxi_gpio_init(led, gpio::GPIO_OUTPUT);
    gpio::sunxi_gpio_write(led, 0);

    uart::sunxi_usart_init(&uart::usart0_dbg, 115200);
    print::init_console();
    uart::sunxi_usart_put(&uart::usart0_dbg, b"Hello\n");
    let clk_fail = clk::sunxi_clk_get_fail_addr();
    if clk_fail != 0 {
        print!("CLK: init timeout at 0x{:08x}\r\n", clk_fail);
    }
    clk::sunxi_clk_dump();

    let mut dram = dram::DramPara::default();
    let dram_size = dram.init_dram();

    let Some(stack_top) = boot::dram_stack_top(dram_size) else {
        print!("BOOT: {}\r\n", boot::BootError::RamTooSmall);
        loop {
            wfe();
        }
    };

    // SAFETY: DRAM is initialised and `stack_top` is its top. The callee never returns, so the
    // abandoned SRAM frame is not used again, and the new stack has room for the FAT path.
    unsafe {
        core::arch::asm!(
            "mov sp, {stack}",
            "b {entry}",
            stack = in(reg) stack_top,
            entry = sym boot_with_dram_stack,
            in("r0") dram_size,
            options(noreturn),
        );
    }
}

#[inline(never)]
extern "C" fn boot_with_dram_stack(dram_size: u32) -> ! {
    let led = gpio::gpio_pin(gpio::PORTD, 22);
    let mut sd_rint = None;
    if let Some(mailbox) = boot::fel_mailbox() {
        print!(
            "FEL: payload image 0x{:08x}, {} bytes, args 0x{:08x}\r\n",
            mailbox.image_addr, mailbox.image_size, mailbox.args_addr
        );
        let err = boot::boot_fel(mailbox, dram_size);
        print!("BOOT: {err}\r\n");
    } else {
        let mut sdio = sdhci::Sdhci::default();
        sdhci::sunxi_sdhci_init(&mut sdio);
        let mut sdcard = sdcard::Sdcard_data::new(sdio);
        sd_rint = Some(sdcard.hci.reg + sdhci::reg::RINT);

        sdcard::sdmmc_init(&mut sdcard);

        if sdcard.online {
            let mut block = [0u8; 512];
            let read = sdcard::sdmmc_blk_read(&mut sdcard, block.as_mut_ptr(), 0, 1);
            print!(
                "SD: read {} block(s), first bytes {:02x?}\r\n",
                read,
                &block[..4]
            );

            let err = boot::boot_sd(&mut sdcard, dram_size);
            print!("BOOT: {err}\r\n");
        }
    }

    //мигаем
    loop {
        if let Some(rint) = sd_rint {
            print!("S=0x{:x}\n", read32(rint));
        }
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
