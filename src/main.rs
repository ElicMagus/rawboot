#![no_std]
#![no_main]

use core::panic::PanicInfo;
extern crate rt;

use rt::entry;
entry!(main);

use rt::cpu;

use crate::io::read32;
use crate::io::write32;
pub mod ccu;
pub mod clk;
pub mod gpio;
pub mod io;
pub mod print;
pub mod sdhci;
pub mod uart;
pub mod dram;

fn main() -> ! {
    uart::init();
    print!("Uart inited\n");
    // set_sp_base();
    let gpio_d = gpio::GPIO_D as *mut gpio::GpioRegT;
    let pin22: u32 = 1 << 22;
    //инициализация порта D ноги 22
    gpio::init(gpio_d, 22, gpio::PD22_Select::Output as u8);
    // print!("PD22 inited\n");
    // let clk=clk::sunxi_clk_get_peri1x_rate();
    // print!("PLL={:?} Hz\r\n",clk);
    clk::init();
    let pr = clk::sunxi_clk_dump();
    print!("CLK={:?} MHz\r\n", pr);
    /* ?? */
    let mut reg32 = read32(0x070901f4);
    if (reg32 & 0x1f) != 0 {
        reg32 = (reg32 & 0xffffff8f) | 0x40;
        reg32 = (reg32 & 0xffffff8f) | 0xc0;
        reg32 = (reg32 & 0xffffff8e) | 0xc0;
        write32(0x070901f4, reg32);
        print!("fix vccio detect value:0x{:X?}\r\n", reg32);
    }

    let addr = 0x0200180Cu32;
	let val  = (1 << 16) | (1 << 0);
	write32(addr, val);
	delay();

    //мигаем
    loop {
        unsafe {
            (*gpio_d).dat ^= pin22;
            delay();
        }
    }
}

#[inline(always)]
fn delay() {
    let mut rnd_addr = 0u32;
    let addr = &mut rnd_addr as *mut u32;
    let mut z: u32 = 10000000;
    unsafe {
        // asm!(
        // "1:",
        // "subs {0}, {0}, #1",
        // "bne 1b",
        // in(reg) i
        // );
        while z > 0 {
            core::ptr::write_volatile(addr, z);
            z -= 1;
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let (location, line, column) = match info.location() {
        Some(loc) => (loc.file(), loc.line(), loc.column()),
        _ => ("???", 0, 0),
    };
    print!(
        "Kernel panic!\n\n\
        Panic location:\n      File '{}', line {}, column {}\n\n\
    ",
        location, line, column,
    );
    cpu::wfe();
    loop {}
}
