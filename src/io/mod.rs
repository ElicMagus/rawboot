#[inline(always)]
pub fn write32(addr:u32, value:u32)
{
    unsafe {        
        core::ptr::write_volatile(addr as *mut u32, value as u32);
    }
}

#[inline(always)]
pub fn read32(addr:u32)->u32
{
    let value:u32;
    unsafe {        
        value = core::ptr::read_volatile(addr as *const u32);                
    }
    value
}