// SPDX-License-Identifier: CC-BY-4.0

//! The runtime's arch control and the loader-image-specific CPU helpers.
//!
//! The timer lives in the shared platform layer (`sunxi-pl`) and is re-exported
//! here; the arch control and the startup part stay in the runtime (ADR-0010).

use core::arch::asm;

pub use sunxi_pl::cpu::{get_arch_counter, time_ms, time_us, udelay};

#[unsafe(no_mangle)]
pub fn get_cpsr() -> u32 {
    let x: u32;
    unsafe {
        asm!("mrs {}, cpsr", out(reg) x);
    }
    x
}

#[unsafe(no_mangle)]
pub fn set_cpsr(reg: u32) {
    unsafe {
        asm!("msr cpsr, {}", in(reg) reg);
    }
}

#[unsafe(no_mangle)]
pub fn get_sp() -> u32 {
    let x: u32;
    unsafe {
        asm!("mov {}, sp", out(reg) x);
    }
    x
}

#[unsafe(no_mangle)]
pub fn set_sp(addr: u32) {
    unsafe {
        asm!("mov sp,  {}", in(reg) addr);
    }
}

/// The size of the loader's SRAM image, from the loader's linker script.
#[unsafe(no_mangle)]
pub fn get_spl_size() -> u32 {
    unsafe extern "C" {
        static __spl_end: u32;
    }
    &raw const __spl_end as u32
}

#[unsafe(no_mangle)]
pub fn get_sys_ticks() -> u32 {
    time_ms()
}
