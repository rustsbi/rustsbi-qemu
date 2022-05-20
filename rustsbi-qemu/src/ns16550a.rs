use core::convert::Infallible;
use embedded_hal::serial::{Read, Write};
use uart_16550::MmioSerialPort;

pub(crate) struct Ns16550a(MmioSerialPort);

impl Ns16550a {
    pub unsafe fn new(base: usize) -> Self {
        Self(MmioSerialPort::new(base))
    }
}

impl Read<u8> for Ns16550a {
    type Error = Infallible;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        Ok(self.0.receive())
    }
}

impl Write<u8> for Ns16550a {
    type Error = Infallible;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.0.send(word);
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        Ok(())
    }
}
