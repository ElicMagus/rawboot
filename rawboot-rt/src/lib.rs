#![no_std]

use core::arch::asm;
use core::ptr;

pub mod cpu;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".head.start")]
pub extern "C" fn _start() -> ! {
    unsafe {
        set_sp_base();

        unsafe extern "C" {
            static mut _sbss: u8;
            static mut _ebss: u8;

            static mut _sdata: u8;
            static mut _edata: u8;
            static _sidata: u8;
        }

        let count = &raw const _ebss as usize - &raw const _sbss as usize;
        ptr::write_bytes(&raw mut _sbss, 0, count);

        let count = &raw const _edata as usize - &raw const _sdata as usize;
        ptr::copy_nonoverlapping(&raw const _sidata, &raw mut _sdata, count);

        unsafe extern "Rust" {
            fn main() -> !;
        }
        main()
    }
}

#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[unsafe(export_name = "main")]
        pub unsafe fn __main() -> ! {
            // type check the given path
            let f: fn() -> ! = $path;

            f()
        }
    };
}

#[unsafe(link_section = ".head.head")]
#[unsafe(no_mangle)]
pub static START_ADR: u32 = 0xEA00000A;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".head.head")]
pub static eGON_BT0: [u8; 8] = *b"eGON.BT0";

#[unsafe(no_mangle)]
#[unsafe(link_section = ".head.head")]
pub static HEAD: [u32; 7] = [
    0x12345678, 32768, 0x30, 0x30303033, 0x00020000, 0x00020000, 0x00,
];

#[unsafe(no_mangle)]
#[unsafe(link_section = ".head.head")]
pub static P_INFO: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x34, 0x2e, 0x30, 0x00];

#[inline(always)]
fn set_sp_base() {
    unsafe {
        let start: u32 = 0x38000;
        asm!(
            "mov sp,  {}",
            in(reg) start,
        );
    }
}
