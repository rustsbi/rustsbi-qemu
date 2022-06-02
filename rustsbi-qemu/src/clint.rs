//! 很久很久以前，CLINT 和 PLIC 都定义在 riscv-privileged 里。
//! 如今，PLIC 有了自己的独立[标准](https://github.com/riscv/riscv-plic-spec)，
//! CLINT 却消失不见了。

use crate::hart_id;
use rustsbi::SbiRet;
use rustsbi::{HartMask, Ipi, Timer};
use spin::Once;

pub(crate) struct Clint {
    base: usize,
}

static CLINT: Once<Clint> = Once::new();

pub(crate) fn init(base: usize) {
    CLINT.call_once(|| Clint { base });
}

pub(crate) fn get() -> &'static Clint {
    CLINT.wait()
}

impl Clint {
    #[inline]
    pub fn get_mtime(&self) -> u64 {
        unsafe { ((self.base as *mut u8).add(0xbff8) as *mut u64).read_volatile() }
    }

    #[inline]
    pub fn send_soft(&self, hart_id: usize) {
        unsafe { (self.base as *mut u32).add(hart_id).write_volatile(1) };
    }

    #[inline]
    pub fn clear_soft(&self, hart_id: usize) {
        unsafe { (self.base as *mut u32).add(hart_id).write_volatile(0) };
    }
}

impl Ipi for Clint {
    #[inline]
    fn send_ipi_many(&self, hart_mask: HartMask) -> SbiRet {
        let hsm = crate::HSM.wait();
        for i in 0..crate::NUM_HART_MAX {
            if hart_mask.has_bit(i) && hsm.is_ipi_allowed(i) {
                self.send_soft(i);
            }
        }
        SbiRet::ok(0)
    }
}

impl Timer for Clint {
    #[inline]
    fn set_timer(&self, time_value: u64) {
        unsafe {
            ((self.base as *mut u8).offset(0x4000) as *mut u64)
                .add(hart_id())
                .write_volatile(time_value);
        }
    }
}
