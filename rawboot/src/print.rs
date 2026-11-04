// SPDX-License-Identifier: CC-BY-4.0

use core::fmt;
use core::fmt::Write;

use sunxi_drivers::console;
use sunxi_drivers::uart;

/// Routes the driver crate's progress messages to USART0.
///
/// Call once, after the UART is initialised and before the drivers log.
pub fn init_console() {
    console::set_console(&uart::UART_CONSOLE);
}

#[unsafe(no_mangle)]
pub fn _print(args: fmt::Arguments) {
    uart::console().write_fmt(args).unwrap();
}

/// Prints without a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::_print(format_args!($($arg)*)));
}

/// Prints with a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::print::_print(format_args!($($arg)*));
    })
}
