use core::fmt::{Arguments, Write};
use spin::{Mutex, Once};
use uart_16550::MmioSerialPort;

static NS16550A: Once<Mutex<MmioSerialPort>> = Once::new();

pub(crate) fn init(base: usize) {
    NS16550A.call_once(|| Mutex::new(unsafe { MmioSerialPort::new(base) }));
}

pub fn print(args: Arguments) {
    NS16550A.wait().lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::print(core::format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {
        $crate::console::print(core::format_args!($($arg)*));
        $crate::print!("\n");
    }
}
