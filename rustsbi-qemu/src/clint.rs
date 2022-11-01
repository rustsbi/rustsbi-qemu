use core::cell::UnsafeCell;
use rustsbi::{spec::binary::SbiRet, HartMask, Ipi, Timer};

pub(crate) struct Clint;

impl Ipi for Clint {
    #[inline]
    fn send_ipi(&self, hart_mask: HartMask) -> SbiRet {
        let hsm = crate::HSM.wait();
        for i in 0..crate::NUM_HART_MAX {
            if hart_mask.has_bit(i) && hsm.is_ipi_allowed(i) {
                msip::send(i);
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
            mtimecmp::set(time_value);
        }
    }
}

static mut BASE: UnsafeCell<usize> = UnsafeCell::new(0);

#[inline]
pub(crate) fn init(base: usize) {
    unsafe { *BASE.get() = base };
}

#[allow(unused)]
pub mod mtime {
    #[inline]
    pub fn read() -> u64 {
        unsafe { ((super::BASE.get().read_volatile() + 0xbff8) as *mut u64).read_volatile() }
    }
}

pub mod mtimecmp {
    #[naked]
    pub unsafe extern "C" fn set_naked(time_value: u64) {
        core::arch::asm!(
            // 保存必要寄存器
            "   addi sp, sp, -16
                sd   t0, 0(sp)
                sd   t1, 8(sp)
            ",
            // 定位并设置当前核的 mtimecmp
            "   li   t1, 0x4000
                la   t0, {base}
                ld   t0, 0(t0)
                add  t0, t0, t1
                csrr t1, mhartid
                slli t1, t1, 3
                add  t0, t0, t1
                sd   a0, 0(t0)
            ",
            // 恢复上下文并返回
            "   ld   t1, 8(sp)
                ld   t0, 0(sp)
                addi sp, sp,  16
                ret
            ",
            base = sym super::BASE,
            options(noreturn)
        )
    }

    #[inline]
    pub fn set(time_value: u64) {
        unsafe { set_naked(time_value) };
    }

    #[inline]
    pub fn clear() {
        unsafe { set_naked(u64::MAX) };
    }
}

pub mod msip {
    #[naked]
    pub unsafe extern "C" fn clear_naked() -> usize {
        core::arch::asm!(
            // 保存必要寄存器
            "   addi sp, sp, -16
                sd   t0, 0(sp)
                sd   t1, 8(sp)
            ",
            // 定位并清除当前核的 msip
            "   la   t0, {base}
                ld   t0, (t0)
                csrr t1, mhartid
                slli t1, t1, 2
                add  t0, t0, t1
                sw   zero, 0(t0)
            ",
            // 恢复上下文并返回
            "   ld   t1, 8(sp)
                ld   t0, 0(sp)
                addi sp, sp,  16
                ret
            ",
            base = sym super::BASE,
            options(noreturn)
        )
    }

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
        unsafe { clear_naked() };
    }
}
