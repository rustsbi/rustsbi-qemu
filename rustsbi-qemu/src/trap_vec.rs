use crate::clint::CLINT;
use aclint::SifiveClint as Clint;
use core::arch::naked_asm;
use fast_trap::trap_entry;

/// 中断向量表
///
/// # Safety
///
/// 裸函数。
#[unsafe(naked)]
pub(crate) unsafe extern "C" fn trap_vec() {
    naked_asm!(
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
    )
}

/// machine timer 中断代理
///
/// # Safety
///
/// 裸函数。
#[unsafe(naked)]
unsafe extern "C" fn mtimer() {
    naked_asm!(
        // 换栈：
        // sp      : M sp
        // mscratch: S sp
        "   csrrw sp, mscratch, sp",
        // 保护
        "   addi  sp, sp, -4*8
            sd    ra, 0*8(sp)
            sd    a0, 1*8(sp)
            sd    a1, 2*8(sp)
            sd    a2, 3*8(sp)
        ",
        // 清除 mtimecmp
        "   la    a0, {clint_ptr}
            ld    a0, (a0)
            csrr  a1, mhartid
            addi  a2, zero, -1
            call  {set_mtimecmp}
        ",
        // 设置 stip
        "   li    a0, {mip_stip}
            csrrs zero, mip, a0
        ",
        // 恢复
        "   ld    ra, 0*8(sp)
            ld    a0, 1*8(sp)
            ld    a1, 2*8(sp)
            ld    a2, 3*8(sp)
            addi  sp, sp,  4*8
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: M sp
        "   csrrw sp, mscratch, sp",
        // 返回
        "   mret",
        mip_stip     = const 1 << 5,
        clint_ptr    =   sym CLINT,
        //                   Clint::write_mtimecmp_naked(&self, hart_idx, val)
        set_mtimecmp =   sym Clint::write_mtimecmp_naked,
    )
}

/// machine soft 中断代理
///
/// # Safety
///
/// 裸函数。
#[unsafe(naked)]
unsafe extern "C" fn msoft() {
    naked_asm!(
        // 换栈：
        // sp      : M sp
        // mscratch: S sp
        "   csrrw sp, mscratch, sp",
        // 保护
        "   addi sp, sp, -3*8
            sd   ra, 0*8(sp)
            sd   a0, 1*8(sp)
            sd   a1, 2*8(sp)
        ",
        // 清除 msip 设置 ssip
        "   la   a0, {clint_ptr}
            ld   a0, (a0)
            csrr a1, mhartid
            call {clear_msip}
            csrrsi zero, mip, 1 << 1
        ",
        // 恢复
        "   ld   ra, 0*8(sp)
            ld   a0, 1*8(sp)
            ld   a1, 2*8(sp)
            addi sp, sp,  3*8
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: M sp
        "   csrrw sp, mscratch, sp",
        // 返回
        "   mret",
        clint_ptr  = sym CLINT,
        //               Clint::clear_msip_naked(&self, hart_idx)
        clear_msip = sym Clint::clear_msip_naked,
    )
}
