use crate::{hart_id, trap_stack::remote_hsm};
use aclint::SifiveClint;
use core::{mem::MaybeUninit, ptr::NonNull};
use rustsbi::{spec::binary::SbiRet, HartMask, Ipi, Timer};

pub(crate) struct Clint;

pub(crate) static mut CLINT: MaybeUninit<NonNull<SifiveClint>> = MaybeUninit::uninit();

pub(crate) fn init(base: usize) {
    unsafe {
        CLINT
            .as_mut_ptr()
            .write_volatile(NonNull::new(base as _).unwrap())
    }
}

impl Ipi for Clint {
    #[inline]
    fn send_ipi(&self, hart_mask: HartMask) -> SbiRet {
        for i in 0..crate::NUM_HART_MAX {
            if hart_mask.has_bit(i) && remote_hsm(i).map_or(false, |hsm| hsm.allow_ipi()) {
                set_msip(i);
            }
        }
        SbiRet::success(0)
    }
}

impl Timer for Clint {
    #[inline]
    fn set_timer(&self, time_value: u64) {
        unsafe {
            riscv::register::mip::clear_stimer();
            clint().write_mtimecmp(hart_id(), time_value);
        }
    }
}

#[inline]
fn clint() -> &'static SifiveClint {
    unsafe { CLINT.as_ptr().read_volatile().as_ref() }
}

#[inline]
pub fn set_msip(hart_idx: usize) {
    clint().set_msip(hart_idx);
}

#[inline]
pub fn clear() {
    let clint = clint();
    clint.clear_msip(hart_id());
    clint.write_mtimecmp(hart_id(), u64::MAX);
}
