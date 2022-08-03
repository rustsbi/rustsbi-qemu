use crate::hart_id;
use core::cell::UnsafeCell;
use rustsbi::{spec::binary::SbiRet, HartMask, Ipi, Timer};

#[repr(transparent)]
pub(crate) struct Clint;

static mut BASE: UnsafeCell<usize> = UnsafeCell::new(0);

#[inline]
pub(crate) fn init(base: usize) {
    #[allow(unused)]
    static CLINT: spin::Once<usize> = spin::Once::new();
    CLINT.call_once(|| base); // FIXME: 一旦删了这行测试就不过了
    unsafe { *BASE.get() = base };
}

#[inline]
pub(crate) fn get() -> &'static Clint {
    &Clint
}

impl Clint {
    #[allow(unused)]
    #[inline]
    pub fn get_mtime(&self) -> u64 {
        unsafe { ((BASE.get().read_volatile() + 0xbff8) as *mut u64).read_volatile() }
    }

    #[inline]
    pub fn set_mtimercomp(&self, value: u64) {
        unsafe {
            ((BASE.get().read_volatile() + 0x4000) as *mut u64)
                .add(hart_id())
                .write_volatile(value)
        }
    }

    #[inline]
    pub fn send_soft(&self, hart_id: usize) {
        unsafe {
            (BASE.get().read_volatile() as *mut u32)
                .add(hart_id)
                .write_volatile(1)
        };
    }

    #[inline]
    pub fn clear_soft(&self, hart_id: usize) {
        unsafe {
            (BASE.get().read_volatile() as *mut u32)
                .add(hart_id)
                .write_volatile(0)
        };
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
            riscv::register::mip::clear_stimer();
            self.set_mtimercomp(time_value);
        }
    }
}
