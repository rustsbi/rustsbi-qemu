use core::fmt;
use spin::Mutex;
use uart_16550::MmioSerialPort;

pub(crate) struct Ns16550a(Mutex<MmioSerialPort>);

impl Ns16550a {
    pub unsafe fn new(base: usize) -> Self {
        Self(Mutex::new(MmioSerialPort::new(base)))
    }

    #[inline]
    pub(crate) fn getchar(&self) -> u8 {
        self.0.lock().receive()
    }

    #[inline]
    pub(crate) fn putchar(&self, ch: u8) {
        self.0.lock().send(ch);
    }
}

impl fmt::Write for &Ns16550a {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.lock().write_str(s).unwrap();
        Ok(())
    }
}
