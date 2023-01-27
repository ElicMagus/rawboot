#![no_std]
#![no_main]

use core::panic::PanicInfo;
extern crate rt;

use rt::entry;
entry!(main);

use rt::cpu;
pub mod ccu;
pub mod clk;
pub mod gpio;
pub mod io;
pub mod print;
pub mod sdhci;
pub mod uart;

fn main() -> ! {
    uart::init();
    print!("Uart inited\n");
    // set_sp_base();
    let gpio_d = gpio::GPIO_D as *mut gpio::GpioRegT;
    let pin22: u32 = 1 << 22;
    //инициализация порта D ноги 22
    gpio::init(gpio_d, 22, gpio::PD22_Select::Output as u8);
    print!("PD22 inited\n");
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
