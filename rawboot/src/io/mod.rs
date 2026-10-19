// SPDX-License-Identifier: Unlicense

#[inline(always)]
pub fn read8(addr: u32) -> u8 {
    unsafe { core::ptr::read_volatile(addr as *const u8) }
}

#[inline(always)]
pub fn read16(addr: u32) -> u16 {
    unsafe { core::ptr::read_volatile(addr as *const u16) }
}

#[inline(always)]
pub fn read32(addr: u32) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

#[inline(always)]
pub fn read64(addr: u32) -> u64 {
    unsafe { core::ptr::read_volatile(addr as *const u64) }
}

#[inline(always)]
pub fn write8(addr: u32, value: u8) {
    unsafe {
        core::ptr::write_volatile(addr as *mut u8, value);
    }
}

#[inline(always)]
pub fn write16(addr: u32, value: u16) {
    unsafe {
        core::ptr::write_volatile(addr as *mut u16, value);
    }
}

#[inline(always)]
pub fn write32(addr: u32, value: u32) {
    unsafe {
        core::ptr::write_volatile(addr as *mut u32, value);
    }
}

#[inline(always)]
pub fn write64(addr: u32, value: u64) {
    unsafe {
        core::ptr::write_volatile(addr as *mut u64, value);
    }
}

/// Simple constant-time spin loop, transcribed from upstream `sdelay`.
///
/// Deliberately not inline: upstream keeps it as an out-of-line function so the
/// same instructions are reused across the many call sites.
#[inline(never)]
pub fn sdelay(loops: u32) {
    unsafe {
        core::arch::asm!(
            "1:",
            "subs {loops}, {loops}, #1",
            "bne 1b",
            loops = inout(reg) loops => _,
            options(nomem, nostack),
        );
    }
}
