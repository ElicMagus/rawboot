// #[inline(always)]
pub fn write32(addr:u32, value:u32)
{
    unsafe {        
        core::ptr::write_volatile(addr as *mut u32, value as u32);
    }
}

// #[inline(always)]
pub fn read32(addr:u32)->u32
{
    let value:u32;
    unsafe {        
        value = core::ptr::read_volatile(addr as *const u32);                
    }
    value
}

pub fn sdelay(ticks:u32) {
    let mut rnd_addr = 0u32;
    let addr = &mut rnd_addr as *mut u32;
    let mut z: u32 = ticks;
    unsafe {
        // asm!(
        // "1:",
        // "subs {0}, {0}, #1",
        // "bne 1b",
        // in(reg) i
        // );
        while z > 0 {
            core::ptr::write_volatile(addr, z);
            z -= 1;
        }
    }
}