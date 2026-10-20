// SPDX-License-Identifier: Unlicense

//! PD22 blink loop for the end-to-end test payload.
//!
//! PD22 is the board LED that `rawboot` drives before handoff. The payload is a
//! separate image, so it configures the pin itself instead of relying on the
//! loader's GPIO state.
//!
//! The cadence is a short blink followed by a long one, then a pause. The
//! loader toggles the same LED every 50 ms, so the payload's pattern is clearly
//! distinguishable on the board without a serial cable.

use core::arch::asm;

/// Port D GPIO register block, as used by `rawboot::gpio`.
const GPIO_PORTD_BASE: u32 = 0x0200_0090;
const GPIO_CFG0: u32 = 0x00;
const GPIO_DAT: u32 = 0x10;

const LED_PIN: u32 = 22;
const GPIO_OUTPUT: u32 = 1;

const GPIO_CFG_PINS_PER_REG: u32 = 8;
const GPIO_CFG_PIN_SHIFT: u32 = 3;
const GPIO_CFG_BITS_PER_PIN: u32 = 4;
const GPIO_CFG_REG_SHIFT: u32 = 2;
const GPIO_CFG_FIELD_SHIFT: u32 = 2;
const GPIO_CFG_MASK: u32 = (1 << GPIO_CFG_BITS_PER_PIN) - 1;
const GPIO_PIN_NUM_MASK: u32 = GPIO_CFG_PINS_PER_REG - 1;

const LED_CFG_ADDR: u32 =
    GPIO_PORTD_BASE + GPIO_CFG0 + ((LED_PIN >> GPIO_CFG_PIN_SHIFT) << GPIO_CFG_REG_SHIFT);
const LED_CFG_SHIFT: u32 = (LED_PIN & GPIO_PIN_NUM_MASK) << GPIO_CFG_FIELD_SHIFT;
const LED_DAT_ADDR: u32 = GPIO_PORTD_BASE + GPIO_DAT;

/// The T113-S3 generic timer runs at 24 MHz, the rate `rawboot-rt` assumes.
const TICKS_PER_MS: u64 = 24_000;

/// `(on_ms, off_ms)` pairs: a short blink, then a long one.
const BLINK_PATTERN: [(u32, u32); 2] = [(120, 180), (600, 900)];

pub fn run() -> ! {
    init();

    loop {
        for &(on_ms, off_ms) in &BLINK_PATTERN {
            set(true);
            delay_ms(on_ms);
            set(false);
            delay_ms(off_ms);
        }
    }
}

fn init() {
    let mut cfg = read32(LED_CFG_ADDR);
    cfg &= !(GPIO_CFG_MASK << LED_CFG_SHIFT);
    cfg |= GPIO_OUTPUT << LED_CFG_SHIFT;
    write32(LED_CFG_ADDR, cfg);
}

fn set(on: bool) {
    let mut value = read32(LED_DAT_ADDR);
    if on {
        value |= 1 << LED_PIN;
    } else {
        value &= !(1 << LED_PIN);
    }
    write32(LED_DAT_ADDR, value);
}

fn delay_ms(ms: u32) {
    let start = ticks();
    let duration = u64::from(ms) * TICKS_PER_MS;
    while ticks().wrapping_sub(start) < duration {}
}

fn ticks() -> u64 {
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

#[inline(always)]
fn read32(addr: u32) -> u32 {
    // SAFETY: `addr` is a GPIO MMIO register in the SoC's peripheral range.
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

#[inline(always)]
fn write32(addr: u32, value: u32) {
    // SAFETY: `addr` is a GPIO MMIO register in the SoC's peripheral range.
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
}
