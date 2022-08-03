use core::cell::UnsafeCell;
use rustsbi::{spec::binary::SbiRet, HartMask, Ipi, Timer};

pub(crate) struct Clint;

impl Ipi for Clint {
    #[inline]
    fn send_ipi_many(&self, hart_mask: HartMask) -> SbiRet {
        let hsm = crate::HSM.wait();
        for i in 0..crate::NUM_HART_MAX {
            if hart_mask.has_bit(i) && hsm.is_ipi_allowed(i) {
                msip::send(i);
            }
        }
        SbiRet::ok(0)
    }
}

impl Timer for Clint {
    #[inline]
    fn set_timer(&self, time_value: u64) {
        unsafe {
            riscv::register::mip::clear_stimer();
            mtimecmp::set(time_value);
        }
    }
}

static mut BASE: UnsafeCell<usize> = UnsafeCell::new(0);

#[inline]
pub(crate) fn init(base: usize) {
    unsafe { *BASE.get() = base };
}

pub mod mtime {
    #[allow(unused)]
    #[inline]
    pub fn read() -> u64 {
        unsafe { ((super::BASE.get().read_volatile() + 0xbff8) as *mut u64).read_volatile() }
    }
}

pub mod mtimecmp {
    #[inline]
    pub fn set(value: u64) {
        unsafe {
            ((super::BASE.get().read_volatile() + 0x4000) as *mut u64)
                .add(crate::hart_id())
                .write_volatile(value)
        }
    }

    #[inline]
    pub fn clear() {
        unsafe {
            ((super::BASE.get().read_volatile() + 0x4000) as *mut u64)
                .add(crate::hart_id())
                .write_volatile(u64::MAX)
        }
    }
}

pub mod msip {
    #[inline]
    pub fn send(hart_id: usize) {
        unsafe {
            (super::BASE.get().read_volatile() as *mut u32)
                .add(hart_id)
                .write_volatile(1)
        };
    }

    #[inline]
    pub fn clear() {
        unsafe {
            (super::BASE.get().read_volatile() as *mut u32)
                .add(crate::hart_id())
                .write_volatile(0)
        };
    }
}
