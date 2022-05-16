use core::fmt::{self, Write};
use spin::Mutex;

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut buffer = [0u8; 4];
        for c in s.chars() {
            for code_point in c.encode_utf8(&mut buffer).as_bytes().iter() {
                sbi::legacy::console_putchar(*code_point as usize);
            }
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    lazy_static::lazy_static! {
        static ref STDOUT: Mutex<Stdout> = Mutex::new(Stdout);
    }

    STDOUT.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::print(core::format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($arg:tt)*) => {
        $crate::console::print(core::format_args!($($arg)*));
        $crate::print!("\r\n");
    }
}
