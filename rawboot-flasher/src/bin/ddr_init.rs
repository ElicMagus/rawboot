// SPDX-License-Identifier: Unlicense

//! FEL-callable DDR-init stub image (`target/rawboot-ddr.bin`).
//!
//! Linked at [`rawboot_image::DDR_STUB_LOAD_ADDR`], loaded with FEL `write`
//! and started with FEL `exec`. It returns to the BROM instead of parking, so
//! the host can keep using the same FEL connection (ADR-0013).

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> u32 {
    // SAFETY: the linker script defines the BSS bounds for this image and the
    // stub owns the SRAM it was loaded into.
    unsafe {
        zero_bss();
    }
    rawboot_flasher::ddr::run()
}

/// Zero the image's BSS section; the flat binary does not carry it.
///
/// # Safety
///
/// `_sbss` and `_ebss` must bracket the writable zero-initialised section laid
/// out by this image's linker script.
unsafe fn zero_bss() {
    unsafe extern "C" {
        static mut _sbss: u8;
        static mut _ebss: u8;
    }

    let mut cursor = &raw mut _sbss;
    let end = &raw mut _ebss;
    while cursor < end {
        // SAFETY: the linker script places the whole range in writable SRAM.
        unsafe {
            ptr::write_volatile(cursor, 0);
            cursor = cursor.add(1);
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
