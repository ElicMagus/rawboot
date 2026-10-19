// SPDX-License-Identifier: Unlicense

use crate::uart;
use core::fmt;

#[unsafe(no_mangle)]
pub fn _print(args: fmt::Arguments) {
    pub use core::fmt::Write;
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
