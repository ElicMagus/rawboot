// SPDX-License-Identifier: Unlicense

//! A stub payload image, wrapped by `cargo xtask payload`.
//!
//! It exists to prove the payload image contract end to end: `xtask` builds this
//! binary, wraps it in the rawboot image header and writes the image; the loader
//! parses that image back. The loader hands off with a valid `SP` and the CPU in
//! Supervisor mode, so a minimal payload needs no startup code. Issue #9 replaces
//! this loop with the blink pattern.

#![no_std]
#![no_main]

use core::hint::spin_loop;
use core::panic::PanicInfo;

/// The payload entry point. It must land at the image header's `entry` (the start of
/// the image), so it is placed in the first section of the linker script.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".payload.start")]
pub extern "C" fn _start() -> ! {
    loop {
        spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        spin_loop();
    }
}
