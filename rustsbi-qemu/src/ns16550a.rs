use core::fmt::Write;
use rustsbi::legacy_stdio::LegacyStdio;
use spin::Mutex;
use uart_16550::MmioSerialPort;

pub(crate) struct Ns16550a(Mutex<MmioSerialPort>);

impl Ns16550a {
    pub unsafe fn new(base: usize) -> Self {
        Self(Mutex::new(MmioSerialPort::new(base)))
    }
}

impl LegacyStdio for Ns16550a {
    fn getchar(&self) -> u8 {
        self.0.lock().receive()
    }

    fn putchar(&self, ch: u8) {
        self.0.lock().send(ch);
    }

    fn write_str(&self, s: &str) {
        self.0.lock().write_str(s).unwrap();
    }
}
