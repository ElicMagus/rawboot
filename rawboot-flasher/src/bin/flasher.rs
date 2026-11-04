// SPDX-License-Identifier: CC-BY-4.0

//! FEL-callable flasher image linked into DRAM (`target/rawboot-flasher.bin`).
//!
//! The host loads it at [`rawboot_image::FLASHER_LOAD_ADDR`] after the DDR-init
//! stub has made DRAM usable, then calls FEL `exec`. The entry switches to the
//! DRAM stack reserved above the image, runs one job, restores the BROM's stack
//! and returns to it (ADR-0013).

#![no_std]
#![no_main]

use core::panic::PanicInfo;

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
        rawboot_flasher::zero_bss();
    }
    rawboot_flasher::flasher::run();
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // Parking is the contract: the BROM is not running, so the host cannot read
    // a status. It observes the FEL timeout instead (ADR-0013).
    loop {
        core::hint::spin_loop();
    }
}
