use crate::clint;
use core::arch::asm;
use fast_trap::trap_entry;

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
        // 需要 a0 传参，保护
        "   addi sp, sp, -16
            sd   ra, 0(sp)
            sd   a0, 8(sp)
        ",
        // clint::mtimecmp::clear();
        "   li   a0, {u64_max}
            call {set_mtimecmp}
        ",
        // mip::set_stimer();
        "   li   a0, {mip_stip}
           csrrs zero, mip, a0
        ",
        // 恢复 a0
        "   ld   a0, 8(sp)
            ld   ra, 0(sp)
            addi sp, sp,  16
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: M sp
        "   csrrw sp, mscratch, sp",
        // 返回
        "   mret",
        u64_max      = const u64::MAX,
        mip_stip     = const 1 << 5,
        set_mtimecmp =   sym clint::mtimecmp::set_naked,
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
        // 保护 ra
        "   addi sp, sp, -8
            sd   ra, 0(sp)
        ",
        // clint::msip::clear();
        // mip::set_ssoft();
        "   call   {clear_msip}
            csrrsi zero, mip, 1 << 1
        ",
        // 恢复 ra
        "   ld   ra, 0(sp)
            addi sp, sp,  8
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: M sp
        "   csrrw sp, mscratch, sp",
        // 返回
        "   mret",
        clear_msip = sym clint::msip::clear_naked,
        options(noreturn)
    )
}
