// SPDX-License-Identifier: Unlicense

//! FEL-callable flasher image linked into DRAM (`target/rawboot-flasher.bin`).
//!
//! The host loads it at [`rawboot_image::FLASHER_LOAD_ADDR`] after the DDR-init
//! stub has made DRAM usable, then calls FEL `exec`. The entry switches to the
//! DRAM stack reserved above the image, runs one job, restores the BROM's stack
//! and returns to it (ADR-0013).

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

/// Entry point: save the BROM's `sp`/`lr`, switch to the DRAM stack, run the
/// job, then restore the caller's registers and return to the BROM.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() {
    core::arch::naked_asm!(
        "push {{r4, lr}}",
        "mov r4, sp",
        "ldr sp, =_estack",
        "bl {run}",
        "mov sp, r4",
        "pop {{r4, lr}}",
        "bx lr",
        run = sym run,
    );
}

extern "C" fn run() {
    // SAFETY: the linker script defines the BSS bounds for this image and the
    // host loaded only this image into its DRAM window.
    unsafe {
        zero_bss();
    }
    rawboot_flasher::flasher::run();
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
        // SAFETY: the linker script places the whole range in writable DRAM.
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
