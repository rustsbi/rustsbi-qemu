use rustsbi::legacy_stdio::LegacyStdio;
use uart_16550::MmioSerialPort;

pub(crate) struct Ns16550a(MmioSerialPort);

impl Ns16550a {
    pub unsafe fn new(base: usize) -> Self {
        Self(MmioSerialPort::new(base))
    }
}

impl LegacyStdio for Ns16550a {
    fn getchar(&mut self) -> u8 {
        self.0.receive()
    }

    fn putchar(&mut self, ch: u8) {
        self.0.send(ch);
    }
}
