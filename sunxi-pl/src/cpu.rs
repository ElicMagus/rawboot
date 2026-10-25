// SPDX-License-Identifier: Unlicense

//! The CPU timer and the wait primitive.
//!
//! The timer is the CP15 physical counter, read with `mrrc p15, 0, …, c14`.
//! Upstream `arch/arm32/mach-t113s3/arch_timer.c:16-51` derives its
//! millisecond and microsecond time from the same counter and runs the timer at
//! 24 MHz (`arch_timer.c:14`); that frequency lives here as the crate's single
//! constant, so the payload does not hardcode it.

#[cfg(target_arch = "arm")]
use core::arch::asm;

/// The T113-S3 generic timer runs at 24 MHz.
pub const TICKS_PER_MS: u64 = 24_000;

/// Timer ticks per microsecond, derived from the timer frequency.
const TICKS_PER_US: u64 = TICKS_PER_MS / 1000;

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub fn wfe() {
    unsafe { asm!("wfe", options(nomem, nostack)) }
}

/// The 64-bit generic timer counter.
#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub fn get_arch_counter() -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: reading the CP15 physical timer counter is safe in Supervisor mode.
    unsafe {
        asm!(
            "mrrc p15, 0, {}, {}, c14",
            out(reg) low,
            out(reg) high,
            options(nomem, nostack),
        );
    }
    (u64::from(high) << 32) | u64::from(low)
}

// The host build exists only so the allocation-free FAT and block code can be
// unit-tested. It has no timer or CPU to wait on, and nothing may run the
// driver entry points there.
#[cfg(not(target_arch = "arm"))]
#[unsafe(no_mangle)]
pub fn wfe() {}

#[cfg(not(target_arch = "arm"))]
#[unsafe(no_mangle)]
pub fn get_arch_counter() -> u64 {
    0
}

#[unsafe(no_mangle)]
pub fn time_ms() -> u32 {
    (get_arch_counter() / TICKS_PER_MS) as u32
}

#[unsafe(no_mangle)]
pub fn time_us() -> u64 {
    get_arch_counter() / TICKS_PER_US
}

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub fn udelay(us: u64) {
    let mut t1 = get_arch_counter();
    let t2 = t1 + us * TICKS_PER_US;
    loop {
        t1 = get_arch_counter();
        if t2 < t1 {
            break;
        }
    }
}

#[cfg(not(target_arch = "arm"))]
#[unsafe(no_mangle)]
pub fn udelay(_us: u64) {}
