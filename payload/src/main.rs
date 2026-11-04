// SPDX-License-Identifier: CC-BY-4.0

//! The blink test payload, wrapped by `cargo xtask payload`.
//!
//! It proves the payload image contract end to end: `xtask` builds this binary,
//! wraps it in the rawboot image header and writes the image; the loader parses
//! that image back, reads it from SD, copies it to DRAM and hands off. The
//! loader leaves the CPU in Supervisor mode with a valid `SP`, so this payload
//! needs no startup code. It blinks PD22 in a short-long cadence to show that
//! the handoff completed.

#![no_std]
#![no_main]

use core::hint::spin_loop;
use core::panic::PanicInfo;

mod led;

/// The payload entry point. It must land at the image header's `entry` (the
/// start of the image), so it is placed in the first section of the linker
/// script.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".payload.start")]
pub extern "C" fn _start() -> ! {
    led::run()
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        spin_loop();
    }
}
