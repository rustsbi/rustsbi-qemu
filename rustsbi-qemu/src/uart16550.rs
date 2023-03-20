use core::{cell::UnsafeCell, ptr::null};
use spin::lock_api::Mutex;

pub(crate) static UART: Mutex<Uart16550Map> = Mutex::new(Uart16550Map(null()));

pub(crate) fn init(base: usize) {
    *UART.lock() = Uart16550Map(base as _);
}

pub struct Uart16550Map(*const Uart16550);

unsafe impl Send for Uart16550Map {}
unsafe impl Sync for Uart16550Map {}

impl Uart16550Map {
    #[inline]
    pub fn get(&self) -> &Uart16550 {
        unsafe { &*self.0 }
    }
}

#[allow(unused)]
pub struct Uart16550 {
    data: UnsafeCell<u8>,
    int_en: UnsafeCell<u8>,
    fifo_ctrl: UnsafeCell<u8>,
    line_ctrl: UnsafeCell<u8>,
    modem_ctrl: UnsafeCell<u8>,
    line_sts: UnsafeCell<u8>,
}

impl Uart16550 {
    pub fn write(&self, data: u8) -> bool {
        const OUTPUT_EMPTY: u8 = 1 << 5;
        return unsafe {
            if self.line_sts.get().read_volatile() & OUTPUT_EMPTY == OUTPUT_EMPTY {
                self.data.get().write_volatile(data);
                true
            } else {
                false
            }
        };
    }

    pub fn read(&self) -> Option<u8> {
        const INPUT_FULL: u8 = 1;
        return unsafe {
            if self.line_sts.get().read_volatile() & INPUT_FULL == INPUT_FULL {
                Some(self.data.get().read_volatile())
            } else {
                None
            }
        };
    }
}
