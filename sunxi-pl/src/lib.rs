// SPDX-License-Identifier: CC-BY-4.0

//! `sunxi-pl` — the shared platform layer for the loader, the runtime and the
//! payload.
//!
//! CMSIS-like and deliberately bare: volatile MMIO accessors, GPIO and the CPU
//! timer. It carries no startup code, no eGON header and no panic handler —
//! those belong to `rawboot-rt` and to whoever links the layer in (ADR-0010).
//!
//! The API is upstream-shaped (`sunxi_gpio_*`, `read32`/`write32`) and still
//! raw: register values and step order follow `szemzoa/awboot`, the Rust shape
//! is ours (ADR-0007).

#![no_std]

pub mod cpu;
pub mod gpio;
pub mod io;
