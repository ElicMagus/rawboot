

pub const GPIO_BASE: u32 = 0x0200_0000;
const MP: u32 = 0x30;
pub const GPIO_B: u32 = GPIO_BASE + MP * 1;
pub const GPIO_C: u32 = GPIO_BASE + MP * 2;
pub const GPIO_D: u32 = GPIO_BASE + MP * 3;
pub const GPIO_E: u32 = GPIO_BASE + MP * 4;
pub const GPIO_F: u32 = GPIO_BASE + MP * 5;
pub const GPIO_G: u32 = GPIO_BASE + MP * 6;

#[derive(Default,Debug)]
#[repr(C)]
pub struct GpioRegT {
    pub cfg: u128,    
    pub dat: u32, //0-31 pin state; 1 bit per pipn
    pub drv: u128,    
    pub pull: u64,    
}

// #[inline(always)]
pub fn init(gpio: *mut GpioRegT, pin: u32, cfg: u8) {
    unsafe {
        // let mut tmp = core::ptr::read_volatile(&(*gpio).cfg as *const u128);
        // tmp &=!(0b1111 << (pin * 4));
        // core::ptr::write_volatile((&(*gpio).cfg as *const _) as *mut _, tmp);

        // let mut tmp = core::ptr::read_volatile(&(*gpio).cfg as *const u128);
        // tmp |=(cfg as u128) << (pin * 4) ;
        // core::ptr::write_volatile((&(*gpio).cfg as *const _) as *mut _, tmp);

        (*gpio).cfg &= !(0b1111 << (pin * 4));
        (*gpio).cfg |= (cfg as u128) << (pin * 4);
    }
}


pub enum PD22_Select {
    Input = 0b0000,
    Output,
    OwaOut,
    IrRx,
    Uart1Rx,
    Pwm7,
    PdEint22 = 10,
    Disable = 0b1111,
}

pub enum PE3_Select {
    Input = 0b0000,
    Output = 0b0001,
    NCSI0_MCLK = 0b0010,
    Uart2Rx = 0b0011,
    TWI0_SDA = 0b0100,
    ClkFanout1 = 0b0101,
    Uart0Rx = 0b0110,
    RMII_TXCK = 0b1000,
    PE_EINT3 = 0b1110,
    Disable = 0b1111,
}

pub enum PE2_Select {
    Input = 0b0000,
    Output = 0b0001,
    NCSI0_PCLK = 0b0010,
    Uart2Tx = 0b0011,
    TWI0_SCK = 0b0100,
    ClkFanout0 = 0b0101,
    Uart0Tx = 0b0110,
    RMII_RXD1 = 0b1000,
    PE_EINT2 = 0b1110,
    Disable = 0b1111,
}