use core::fmt::{Arguments, Write};
use log::{Level, LevelFilter, Log};
use spin::{Mutex, Once};
use uart_16550::MmioSerialPort;

static NS16550A: Once<Mutex<MmioSerialPort>> = Once::new();

struct Logger;

pub(crate) fn init(base: usize) {
    NS16550A.call_once(|| Mutex::new(unsafe { MmioSerialPort::new(base) }));
    log::set_logger(&Logger).unwrap();
    log::set_max_level(LevelFilter::Trace);
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
    ($($arg:tt)*) => {{
        $crate::console::print(core::format_args!($($arg)*));
        $crate::print!("\n");
    }}
}

impl Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let color_code = match record.level() {
                Level::Error => "31",
                Level::Warn => "93",
                Level::Info => "34",
                Level::Debug => "32",
                Level::Trace => "90",
            };
            println!(
                "\x1b[{}m[{:>5}] {}\x1b[0m",
                color_code,
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
