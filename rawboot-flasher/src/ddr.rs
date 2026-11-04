// SPDX-License-Identifier: CC-BY-4.0

//! The DDR-init stub, callable through FEL `exec`.
//!
//! It runs from FEL-mode SRAM, initialises DRAM with the loader's own
//! [`sunxi_drivers::dram`] driver, publishes the detected size in SRAM, and
//! returns to the BROM. Returning is the point of the spike: the host needs a
//! live FEL connection afterwards to place the flasher and its job (ADR-0013).

use core::arch::asm;

use rawboot_image::{DDR_RESULT_ADDR, DdrResult};
use sunxi_drivers::dram::DramPara;
use sunxi_pl::io::write8;

/// Initialise DRAM and publish the result for the host.
///
/// Returns the detected size in MiB (zero on failure) in `r0`, and writes the
/// same value as a [`DdrResult`] at [`DDR_RESULT_ADDR`] so the host can
/// distinguish "stub returned" from "stub failed".
pub fn run() -> u32 {
    let mut dram = DramPara::default();
    let size_mib = dram.init_dram();

    for (index, byte) in DdrResult::new(size_mib).to_bytes().iter().enumerate() {
        write8(DDR_RESULT_ADDR + index as u32, *byte);
    }
    // Make the result visible before returning to the BROM, which will answer
    // the host's FEL read from a different context.
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }

    size_mib
}
