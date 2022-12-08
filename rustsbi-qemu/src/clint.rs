use crate::{hart_id, trap_stack::remote_hsm};
use aclint::SifiveClint;
use core::{
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};
use rustsbi::{spec::binary::SbiRet, HartMask, Ipi, Timer};

pub(crate) struct Clint;

pub(crate) static CLINT: AtomicPtr<SifiveClint> = AtomicPtr::new(null_mut());

pub(crate) fn init(base: usize) {
    CLINT.store(base as _, Ordering::Release);
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
            (*CLINT.load(Ordering::Relaxed)).write_mtimecmp(hart_id(), time_value);
        }
    }
}

#[inline]
pub fn set_msip(hart_idx: usize) {
    unsafe { &*CLINT.load(Ordering::Relaxed) }.set_msip(hart_idx);
}

#[inline]
pub fn clear_msip() {
    unsafe { &*CLINT.load(Ordering::Relaxed) }.clear_msip(hart_id());
}

#[inline]
pub fn clear() {
    loop {
        if let Some(clint) = unsafe { CLINT.load(Ordering::Relaxed).as_ref() } {
            clint.clear_msip(hart_id());
            clint.write_mtimecmp(hart_id(), u64::MAX);
            break;
        } else {
            continue;
        }
    }
}
