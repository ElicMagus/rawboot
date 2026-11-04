// SPDX-License-Identifier: CC-BY-4.0

//! The narrow console interface for driver progress messages.
//!
//! The drivers never depend on a particular UART or on the loader's `print!`:
//! they format into the [`Console`] the linking binary installs. A binary that
//! does not install one still gets the driver behaviour, silently.

use core::cell::UnsafeCell;
use core::fmt;
use core::fmt::Write as _;

/// A sink for driver progress messages.
pub trait Console: Sync {
    fn write_str(&self, s: &str);
}

struct Slot(UnsafeCell<Option<&'static dyn Console>>);

// SAFETY: the drivers run on one core during early boot. The binary installs
// the console once, before the drivers log, and only reads it afterwards.
unsafe impl Sync for Slot {}

static SLOT: Slot = Slot(UnsafeCell::new(None));

/// Installs the console the drivers print to.
///
/// Call once, before the first driver call that may log.
pub fn set_console(console: &'static dyn Console) {
    // SAFETY: installation happens once, on a single core, before any driver
    // reads the slot.
    unsafe { *SLOT.0.get() = Some(console) };
}

struct Writer<'a>(&'a dyn Console);

impl fmt::Write for Writer<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s);
        Ok(())
    }
}

/// Writes a driver message to the installed console, if there is one.
///
/// The [`info!`](crate::console::info) macro in this module calls it; driver
/// code should use the macro rather than this function.
/// Deliberately not inline: one copy of the formatting path serves every
/// driver message instead of being duplicated at each call site.
#[inline(never)]
pub fn print(args: fmt::Arguments<'_>) {
    // SAFETY: the slot is written once before any driver runs and only read
    // here; see `set_console`.
    if let Some(console) = unsafe { *SLOT.0.get() } {
        let _ = Writer(console).write_fmt(args);
    }
}

macro_rules! info {
    ($($arg:tt)*) => ($crate::console::print(format_args!($($arg)*)));
}

pub(crate) use info;
