#![allow(unused)]

use crate::uart16550;
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
            let uart = uart16550::UART.lock();
            for ptr in start..end {
                let c = unsafe { (ptr as *const u8).read_volatile() };
                if !uart.get().write(c) {
                    return SbiRet::success(ptr - start);
                }
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
            let uart = uart16550::UART.lock();
            for ptr in start..end {
                if let Some(c) = uart.get().read() {
                    unsafe { (ptr as *mut u8).write_volatile(c) };
                } else {
                    return SbiRet::success(ptr - start);
                }
            }
            SbiRet::success(bytes.num_bytes())
        } else {
            SbiRet::invalid_param()
        }
    }

    #[inline]
    fn write_byte(&self, byte: u8) -> SbiRet {
        let uart = uart16550::UART.lock();
        loop {
            if uart.get().write(byte) {
                return SbiRet::success(0);
            }
        }
    }
}
