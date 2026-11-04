// SPDX-License-Identifier: CC-BY-4.0

//! FEL-callable DDR-init stub image (`target/rawboot-ddr.bin`).
//!
//! Linked at [`rawboot_image::DDR_STUB_LOAD_ADDR`], loaded with FEL `write`
//! and started with FEL `exec`. It returns to the BROM instead of parking, so
//! the host can keep using the same FEL connection (ADR-0013).

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> u32 {
    // SAFETY: the linker script defines the BSS bounds for this image and the
    // stub owns the SRAM it was loaded into.
    unsafe {
        rawboot_flasher::zero_bss();
    }
    rawboot_flasher::ddr::run()
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // Parking is the contract: the BROM is not running, so the host cannot read
    // a result. It observes the FEL timeout instead (ADR-0013).
    loop {
        core::hint::spin_loop();
    }
}
