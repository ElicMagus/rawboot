// SPDX-License-Identifier: Unlicense

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

/// The 48-byte eGON boot header BROM validates, laid down at the very start of
/// the raw image by the `.head.head` section.
///
/// It is a single `#[repr(C)]` value on purpose: separate statics in the same
/// link section are not guaranteed to keep their source order, and a scrambled
/// header makes the image unbootable. The field offsets are asserted below.
#[repr(C)]
struct EgonHead {
    /// ARM branch over the header to `_start` (`0xEA00000A`).
    instruction: u32,
    /// `eGON.BT0`, checked by BROM.
    magic: [u8; 8],
    /// Checksum placeholder; `seal` rewrites it.
    checksum: u32,
    /// Image length placeholder; `seal` rewrites it.
    length: u32,
    /// Spl signature (`0x30`).
    spl_signature: u32,
    /// Boot header version (`0x30303033`).
    boot_header_version: u32,
    /// Return value (`0x00020000`).
    return_value: u32,
    /// Run address (`0x00020000`).
    run_address: u32,
    /// eGON version.
    egon_version: u32,
    /// Platform information.
    platform: [u8; 4],
    /// String pool prefix (`4.0\0`).
    string: [u8; 4],
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".head.head")]
static EGON_HEAD: EgonHead = EgonHead {
    instruction: 0xEA00_000A,
    magic: *b"eGON.BT0",
    checksum: 0x1234_5678,
    length: 32768,
    spl_signature: 0x30,
    boot_header_version: 0x3030_3033,
    return_value: 0x0002_0000,
    run_address: 0x0002_0000,
    egon_version: 0,
    platform: [0x00, 0x00, 0x00, 0x00],
    string: [0x34, 0x2e, 0x30, 0x00],
};

const _: () = {
    use core::mem::{offset_of, size_of};
    assert!(size_of::<EgonHead>() == 48);
    assert!(offset_of!(EgonHead, instruction) == 0x00);
    assert!(offset_of!(EgonHead, magic) == 0x04);
    assert!(offset_of!(EgonHead, checksum) == 0x0c);
    assert!(offset_of!(EgonHead, length) == 0x10);
    assert!(offset_of!(EgonHead, platform) == 0x28);
    assert!(offset_of!(EgonHead, string) == 0x2c);
};

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
