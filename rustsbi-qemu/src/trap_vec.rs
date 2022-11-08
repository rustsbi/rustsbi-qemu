use core::arch::asm;
use fast_trap::trap_entry;
use riscv::register::mtvec::{self, TrapMode::*};
use riscv_aclint::SifiveClint as Clint;

/// 加载陷入向量。
#[inline]
pub(crate) fn load(vec: bool) {
    unsafe { mtvec::write(trap_vec as _, if vec { Vectored } else { Direct }) };
}

/// 中断向量表
///
/// # Safety
///
/// 裸函数。
#[naked]
pub(crate) unsafe extern "C" fn trap_vec() {
    asm!(
        ".align 2",
        ".option push",
        ".option norvc",
        "j {default}", // exception
        "j {default}", // supervisor software
        "j {default}", // reserved
        "j {msoft} ",  // machine    software
        "j {default}", // reserved
        "j {default}", // supervisor timer
        "j {default}", // reserved
        "j {mtimer}",  // machine    timer
        "j {default}", // reserved
        "j {default}", // supervisor external
        "j {default}", // reserved
        "j {default}", // machine    external
        ".option pop",
        default = sym trap_entry,
        mtimer  = sym mtimer,
        msoft   = sym msoft,
        options(noreturn)
    )
}

/// machine timer 中断代理
///
/// # Safety
///
/// 裸函数。
#[naked]
unsafe extern "C" fn mtimer() {
    asm!(
        // 换栈：
        // sp      : M sp
        // mscratch: S sp
        "   csrrw sp, mscratch, sp",
        // 保护
        "   sd    ra, -1*8(sp)
            sd    a0, -2*8(sp)
            sd    a1, -3*8(sp)
            sd    a2, -4*8(sp)
        ",
        // 清除 mtimecmp
        "   la    a0, {clint_ptr}
            ld    a0, (a0)
            csrr  a1, mhartid
            li    a2, {u64_max}
            call  {set_mtimecmp}
        ",
        // 设置 stip
        "   li    a0, {mip_stip}
            csrrs zero, mip, a0
        ",
        // 恢复
        "   ld    ra, -1*8(sp)
            ld    a0, -2*8(sp)
            ld    a1, -3*8(sp)
            ld    a2, -4*8(sp)
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: M sp
        "   csrrw sp, mscratch, sp",
        // 返回
        "   mret",
        u64_max      = const u64::MAX,
        mip_stip     = const 1 << 5,
        clint_ptr    =   sym crate::clint::CLINT,
        //                   Clint::write_mtimecmp_naked(&self, hart_idx, val)
        set_mtimecmp =   sym Clint::write_mtimecmp_naked,
        options(noreturn)
    )
}

/// machine soft 中断代理
///
/// # Safety
///
/// 裸函数。
#[naked]
unsafe extern "C" fn msoft() {
    asm!(
        // 换栈：
        // sp      : M sp
        // mscratch: S sp
        "   csrrw sp, mscratch, sp",
        // 保护
        "   sd   ra, -1*8(sp)
            sd   a0, -2*8(sp)
            sd   a1, -3*8(sp)
        ",
        // 清除 msip 设置 ssip
        "   la   a0, {clint_ptr}
            ld   a0, (a0)
            csrr a1, mhartid
            call {clear_msip}
            csrrsi zero, mip, 1 << 1
        ",
        // 恢复
        "   ld   ra, -1*8(sp)
            ld   a0, -2*8(sp)
            ld   a1, -3*8(sp)
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: M sp
        "   csrrw sp, mscratch, sp",
        // 返回
        "   mret",
        clint_ptr  = sym crate::clint::CLINT,
        //               Clint::clear_msip_naked(&self, hart_idx)
        clear_msip = sym Clint::clear_msip_naked,
        options(noreturn)
    )
}
