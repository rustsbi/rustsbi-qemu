#![allow(dead_code)]

use rustsbi::{HartMask, Ipi, Timer};
// 这部分其实是运行时提供的，不应该做到实现库里面
use rustsbi::SbiRet;

pub struct Clint {
    base: usize,
}

impl Clint {
    #[inline]
    pub fn new(base: *mut u8) -> Clint {
        Clint {
            base: base as usize,
        }
    }

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
        let num_harts = *crate::count_harts::NUM_HARTS.lock();
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
