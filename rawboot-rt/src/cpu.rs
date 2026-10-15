use core::arch::asm;

#[unsafe(no_mangle)]
pub fn wfe() {
    unsafe { asm!("wfe", options(nomem, nostack)) }
}

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
pub fn get_spl_size() -> u32 {
    unsafe extern "C" {
        static __spl_end: u32;
    }
    &raw const __spl_end as u32
}

#[unsafe(no_mangle)]
pub fn get_arch_counter() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "mrrc p15, 0, {}, {}, c14",
            out(reg) low,
            out(reg) high,
        );
    }
    ((high as u64) << 32) | low as u64
}

#[unsafe(no_mangle)]
pub fn get_sys_ticks() -> u32 {
    (get_arch_counter() / 24000) as u32
}

#[unsafe(no_mangle)]
pub fn udelay(us: u64) {
    let mut t1 = get_arch_counter();
    let t2 = t1 + us * 24;
    loop {
        t1 = get_arch_counter();
        if t2 < t1 {
            break;
        }
    }
}

#[unsafe(no_mangle)]
pub fn set_sp(addr: u32) {
    unsafe {
        asm!("mov sp,  {}", in(reg) addr);
    }
}
