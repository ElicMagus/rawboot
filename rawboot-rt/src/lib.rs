// SPDX-License-Identifier: Unlicense

#![no_std]

use core::arch::asm;
use core::ptr;

use rawboot_image::EgonHeader;

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

/// The 48-byte eGON boot header BROM validates, laid down at the very start of
/// the raw image by the `.head.head` section.
///
/// It is a single `#[repr(C)]` value on purpose: separate statics in the same
/// link section are not guaranteed to keep their source order, and a scrambled
/// header makes the image unbootable. The layout lives once in
/// [`rawboot_image::EgonHeader`], whose field offsets are asserted there.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".head.head")]
static EGON_HEAD: EgonHeader = EgonHeader::PLACEHOLDER;

#[inline(always)]
fn set_sp_base() {
    unsafe extern "C" {
        static _estack: u8;
    }
    unsafe {
        asm!(
            "mov sp, {}",
            in(reg) &raw const _estack as usize,
        );
    }
}
