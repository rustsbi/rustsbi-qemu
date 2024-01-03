use core::ptr::null;
use spin::lock_api::Mutex;
use uart16550::Uart16550;

pub(crate) static UART: Mutex<Uart16550Map> = Mutex::new(Uart16550Map(null()));

pub(crate) fn init(base: usize) {
    *UART.lock() = Uart16550Map(base as _);
}

pub struct Uart16550Map(*const Uart16550<u8>);

unsafe impl Send for Uart16550Map {}
unsafe impl Sync for Uart16550Map {}

impl Uart16550Map {
    #[inline]
    pub fn get(&self) -> &Uart16550<u8> {
        unsafe { &*self.0 }
    }
}
