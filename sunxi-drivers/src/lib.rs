// SPDX-License-Identifier: CC-BY-4.0

//! `sunxi-drivers` — the Allwinner T113-S3 drivers shared by the loader and by
//! any future image that links them.
//!
//! The crate sits above the platform layer (`sunxi-pl`) and below a program:
//! clock and DRAM bring-up, UART, SDHCI/SD/MMC, and the FAT adapter (read-only in
//! the loader, read-write for the flasher and the host tests).
//! It carries no startup code, no eGON header, no linker scripts and no global
//! allocator; those belong to the image that links it (ADR-0012).
//!
//! Progress is reported through the narrow [`console::Console`] interface. A
//! binary installs its own sink (the loader uses USART0); without one the
//! drivers still do their work, silently.

#![cfg_attr(not(test), no_std)]

pub mod ccu;
pub mod clk;
pub mod console;
pub mod dram;
pub mod fat;
pub mod sdcard;
pub mod sdhci;
pub mod uart;
