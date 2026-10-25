// SPDX-License-Identifier: Unlicense

#![no_std]

//! `rawboot-flasher` — the two FEL-callable images of the in-board flasher path.
//!
//! The host first runs [`ddr::run`] (through the `rawboot-ddr-init` binary) to
//! initialise DRAM; the stub writes its result to SRAM and returns to the BROM,
//! leaving FEL usable. The host then loads the `rawboot-flasher` binary into
//! DRAM and executes it. That binary reads a [`rawboot_image::FlashJob`] from a
//! fixed DRAM address; the `NOOP` command proves the path, and
//! `FLASH_COMMAND_FLASH_SD` writes the staged payload files and loader to the
//! card. It writes a [`rawboot_image::FlashStatus`] for the host and returns to
//! the BROM as well. Neither image carries an eGON header or is a second-stage
//! loader; see ADR-0013 and ADR-0014.
//!
//! Both binaries zero their BSS before calling into this crate, so the driver
//! and status code sees a deterministic state without a runtime.

pub mod ddr;
pub mod flasher;

/// Zero the BSS section of the image that links this function.
///
/// # Safety
///
/// The image's linker script must define `_sbss` and `_ebss` bracketing a
/// writable zero-initialised region. The flat binary does not carry `.bss`, so
/// every entry point calls this before running driver or contract code.
pub unsafe fn zero_bss() {
    unsafe extern "C" {
        static mut _sbss: u8;
        static mut _ebss: u8;
    }

    let mut cursor = &raw mut _sbss;
    let end = &raw mut _ebss;
    while cursor < end {
        // SAFETY: the caller's linker script places the range in writable RAM.
        unsafe {
            core::ptr::write_volatile(cursor, 0);
            cursor = cursor.add(1);
        }
    }
}
