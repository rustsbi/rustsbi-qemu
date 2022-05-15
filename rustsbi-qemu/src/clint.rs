//! 很久很久以前，CLINT 和 PLIC 都定义在 riscv-privileged 里。
//! 如今，PLIC 有了自己的独立[标准](https://github.com/riscv/riscv-plic-spec)，
//! CLINT 却消失不见了。

use rustsbi::SbiRet;
use rustsbi::{HartMask, Ipi, Timer};
use spin::Once;

#[derive(Clone, Copy)]
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
        unsafe {
            let base = self.base as *mut u8;
            core::ptr::read_volatile(base.add(0xbff8) as *mut u64)
        }
    }

    #[inline]
    pub fn set_timer(&self, hart_id: usize, instant: u64) {
        unsafe {
            let base = self.base as *mut u8;
            core::ptr::write_volatile((base.offset(0x4000) as *mut u64).add(hart_id), instant);
        }
    }

    #[inline]
    pub fn send_soft(&self, hart_id: usize) {
        unsafe {
            let base = self.base as *mut u8;
            core::ptr::write_volatile((base as *mut u32).add(hart_id), 1);
        }
    }

    #[inline]
    pub fn clear_soft(&self, hart_id: usize) {
        unsafe {
            let base = self.base as *mut u8;
            core::ptr::write_volatile((base as *mut u32).add(hart_id), 0);
        }
    }
}

impl Ipi for Clint {
    #[inline]
    fn send_ipi_many(&self, hart_mask: HartMask) -> SbiRet {
        // println!("[rustsbi] send ipi many, {:?}", hart_mask);
        let num_harts = crate::device_tree::get().smp;
        for i in 0..num_harts {
            if hart_mask.has_bit(i) {
                self.send_soft(i);
            }
        }
        SbiRet::ok(0)
    }
}

impl Timer for Clint {
    #[inline]
    fn set_timer(&self, time_value: u64) {
        let this_mhartid = riscv::register::mhartid::read();
        self.set_timer(this_mhartid, time_value);
    }
}
