// SPDX-License-Identifier: Unlicense

//! The DDR-init stub, callable through FEL `exec`.
//!
//! It runs from FEL-mode SRAM, initialises DRAM with the loader's own
//! [`sunxi_drivers::dram`] driver, publishes the detected size in SRAM, and
//! returns to the BROM. Returning is the point of the spike: the host needs a
//! live FEL connection afterwards to place the flasher and its job (ADR-0013).

use core::arch::asm;

use rawboot_image::{DDR_RESULT_ADDR, DDR_RESULT_MAGIC};
use sunxi_drivers::dram::DramPara;
use sunxi_pl::io::write32;

/// Initialise DRAM and publish the result for the host.
///
/// Returns the detected size in MiB (zero on failure) in `r0`, and writes the
/// same value after [`DDR_RESULT_MAGIC`] at [`DDR_RESULT_ADDR`] so the host can
/// distinguish "stub returned" from "stub failed".
pub fn run() -> u32 {
    let mut dram = DramPara::default();
    let size_mib = dram.init_dram();

    write32(DDR_RESULT_ADDR, DDR_RESULT_MAGIC);
    write32(DDR_RESULT_ADDR + 4, size_mib);
    // Make the result visible before returning to the BROM, which will answer
    // the host's FEL read from a different context.
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }

    size_mib
}
