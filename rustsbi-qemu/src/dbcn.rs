#![allow(unused)]

use crate::UART;
use core::ops::Range;
use rustsbi::{spec::binary::SbiRet, Console, Physical};
use spin::Once;

pub(crate) struct DBCN(Range<usize>);

static INSTANCE: Once<DBCN> = Once::new();

pub(crate) fn init(memory: Range<usize>) {
    INSTANCE.call_once(|| DBCN(memory));
}

pub(crate) fn get() -> &'static DBCN {
    INSTANCE.wait()
}

impl Console for DBCN {
    fn write(&self, bytes: Physical<&[u8]>) -> SbiRet {
        let start = bytes.phys_addr_lo();
        let end = start + bytes.num_bytes();
        if self.0.contains(&start) && self.0.contains(&(end - 1)) {
            let mut uart = UART.lock();
            let uart = unsafe { uart.assume_init_mut() };
            for ptr in start..end {
                let c = ptr as *const u8;
                unsafe { uart.send(c.read_volatile()) };
            }
            SbiRet::success(bytes.num_bytes())
        } else {
            SbiRet::invalid_param()
        }
    }

    fn read(&self, bytes: Physical<&mut [u8]>) -> SbiRet {
        let start = bytes.phys_addr_lo();
        let end = start + bytes.num_bytes();
        if self.0.contains(&start) && self.0.contains(&(end - 1)) {
            let mut uart = UART.lock();
            let uart = unsafe { uart.assume_init_mut() };
            for ptr in start..end {
                let c = ptr as *mut u8;
                unsafe { c.write_volatile(uart.receive()) };
            }
            SbiRet::success(bytes.num_bytes())
        } else {
            SbiRet::invalid_param()
        }
    }

    #[inline]
    fn write_byte(&self, byte: u8) -> SbiRet {
        let mut uart = UART.lock();
        let uart = unsafe { uart.assume_init_mut() };
        uart.send(byte);
        SbiRet::success(0)
    }
}
