// SPDX-License-Identifier: Unlicense

#![no_std]

//! `rawboot-flasher` — the two FEL-callable images of the in-board flasher path.
//!
//! The host first runs [`ddr::run`] (through the `rawboot-ddr-init` binary) to
//! initialise DRAM; the stub writes its result to SRAM and returns to the BROM,
//! leaving FEL usable. The host then loads the `rawboot-flasher` binary into
//! DRAM and executes it. That binary reads a [`rawboot_image::FlashJob`] from a
//! fixed DRAM address, writes a [`rawboot_image::FlashStatus`] for the host and
//! returns to the BROM as well. Neither image carries an eGON header or is a
//! second-stage loader; see ADR-0013.
//!
//! Both binaries zero their BSS before calling into this crate, so the driver
//! and status code sees a deterministic state without a runtime.

pub mod ddr;
pub mod flasher;
