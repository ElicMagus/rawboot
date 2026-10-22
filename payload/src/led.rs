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
//!
//! The GPIO and timer access comes from the shared platform layer
//! (`sunxi-pl`), the same code the loader uses (ADR-0010).

use sunxi_pl::cpu::{TICKS_PER_MS, get_arch_counter};
use sunxi_pl::gpio::{GPIO_OUTPUT, GpioPin, PORTD, gpio_pin, sunxi_gpio_init, sunxi_gpio_write};

/// The board LED: port D, pin 22.
const LED: GpioPin = gpio_pin(PORTD, 22);

/// `(on_ms, off_ms)` pairs: a short blink, then a long one.
const BLINK_PATTERN: [(u32, u32); 2] = [(120, 180), (600, 900)];

pub fn run() -> ! {
    sunxi_gpio_init(LED, GPIO_OUTPUT);

    loop {
        for &(on_ms, off_ms) in &BLINK_PATTERN {
            set(true);
            delay_ms(on_ms);
            set(false);
            delay_ms(off_ms);
        }
    }
}

fn set(on: bool) {
    sunxi_gpio_write(LED, on as i32);
}

fn delay_ms(ms: u32) {
    let start = get_arch_counter();
    let duration = u64::from(ms) * TICKS_PER_MS;
    while get_arch_counter().wrapping_sub(start) < duration {}
}
