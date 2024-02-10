use crate::uart16550;
use core::ops::Range;
use rustsbi::{Console, Physical, SbiRet};
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
            let buf = unsafe { core::slice::from_raw_parts(start as *const u8, bytes.num_bytes()) };
            SbiRet::success(uart16550::UART.lock().get().write(buf))
        } else {
            SbiRet::invalid_param()
        }
    }

    fn read(&self, bytes: Physical<&mut [u8]>) -> SbiRet {
        let start = bytes.phys_addr_lo();
        let end = start + bytes.num_bytes();
        if self.0.contains(&start) && self.0.contains(&(end - 1)) {
            let buf =
                unsafe { core::slice::from_raw_parts_mut(start as *mut u8, bytes.num_bytes()) };
            SbiRet::success(uart16550::UART.lock().get().read(buf))
        } else {
            SbiRet::invalid_param()
        }
    }

    #[inline]
    fn write_byte(&self, byte: u8) -> SbiRet {
        let uart = uart16550::UART.lock();
        loop {
            if uart.get().write(&[byte]) == 1 {
                return SbiRet::success(0);
            }
        }
    }
}
