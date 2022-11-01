use crate::ns16550a::Ns16550a;
use core::fmt::{Arguments, Write};
use spin::Once;

pub(crate) static STDOUT: Once<Ns16550a> = Once::new();

pub(crate) fn init(out: Ns16550a) {
    STDOUT.call_once(|| out);
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    STDOUT.wait().write_fmt(args).unwrap();
}

/// Prints to the debug output.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(core::format_args!($($arg)*)));
}

/// Prints to the debug output with a new line.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($arg:tt)*) => {
        $crate::console::_print(core::format_args!($($arg)*));
        $crate::print!("\r\n");
    }
}
