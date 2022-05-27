use crate::{
    clint, hart_id,
    qemu_hsm::{QemuHsm, SUSPEND_RETENTIVE},
    Supervisor,
};

mod context;
mod transfer_trap;

use context::Context;

pub(crate) fn execute_supervisor(hsm: &QemuHsm, supervisor: Supervisor) {
    use core::arch::asm;
    use riscv::register::{medeleg, mie, mstatus};

    unsafe {
        mstatus::set_mpp(mstatus::MPP::Supervisor);
        mstatus::set_mie();
    };

    let mut ctx = Context::new(supervisor);

    clint::get().clear_soft(hart_id());
    unsafe {
        asm!("csrw     mip, {}", in(reg) 0);
        asm!("csrw mideleg, {}", in(reg) usize::MAX);
        asm!("csrw medeleg, {}", in(reg) usize::MAX);
        mstatus::clear_mie();
        medeleg::clear_illegal_instruction();
        medeleg::clear_supervisor_env_call();
        medeleg::clear_machine_env_call();

        crate::set_mtvec(s_to_m as usize);
        mie::set_mext();
        mie::set_msoft();
    }

    hsm.record_current_start_finished();
    loop {
        use crate::qemu_hsm::{EID_HSM, FID_HART_STOP, FID_HART_SUSPEND};
        use riscv::register::{
            mcause::{self, Exception as E, Interrupt as I, Trap as T},
            mip,
        };

        unsafe { m_to_s(&mut ctx) };

        match mcause::read().cause() {
            T::Interrupt(I::MachineTimer) => unsafe {
                mie::clear_mtimer();
                mip::clear_mtimer();
                mip::set_stimer();
            },
            T::Interrupt(I::MachineSoft) => {
                crate::clint::get().clear_soft(hart_id());
                unsafe {
                    mip::clear_msoft();
                    mip::set_ssoft();
                }
            }
            T::Exception(E::SupervisorEnvCall) => {
                let param = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
                let ans = rustsbi::ecall(ctx.a(7), ctx.a(6), param);
                if ctx.a(7) == EID_HSM && ans.error == 0 {
                    match ctx.a(6) {
                        FID_HART_STOP => return,
                        FID_HART_SUSPEND if ctx.a(0) == SUSPEND_RETENTIVE => return,
                        _ => {}
                    }
                }
                *ctx.a_mut(0) = ans.error;
                *ctx.a_mut(1) = ans.value;
                ctx.mepc = ctx.mepc.wrapping_add(4);
            }
            T::Exception(E::IllegalInstruction) => {
                use riscv::register::scause;

                // const OPCODE_MASK: usize = (1 << 7) - 1;
                // const REG_MASK: usize = (1 << 5) - 1;
                // const OPCODE_CSR: usize = 0b1110011;
                // const CSR_TIME: usize = 0xc01;
                // let instruction = mtval::read();
                // 标准 20191213 的表 24.3 列出了一些特殊的 CSR，SBI 软件负责将它们模拟出来
                // Qemu 似乎不需要模拟 time
                // if let OPCODE_CSR = instruction & OPCODE_MASK {
                //     if instruction >> 20 == CSR_TIME {
                //         match (instruction >> 7) & REG_MASK {
                //             0 => {}
                //             rd => *ctx.x_mut(rd) = crate::clint::get().get_mtime() as _,
                //         }
                //         continue;
                //     }
                // }
                // 如果不是可修正的指令，且不是 M 态本身发出的，转交给 S 态处理
                // mpp != machine
                if transfer_trap::should_transfer_trap(&ctx) {
                    transfer_trap::do_transfer_trap(
                        &mut ctx,
                        scause::Trap::Exception(scause::Exception::IllegalInstruction),
                    );
                } else {
                    println!("{:?}", I::MachineSoft);
                    break;
                }
            }
            t => {
                println!("{t:?}");
                break;
            }
        }
    }
    loop {
        core::hint::spin_loop();
    }
}

/// M 态转到 S 态。
///
/// # Safety
///
/// 裸函数，手动保存所有上下文环境。
/// 为了写起来简单，占 32 * usize 空间，循环 31 次保存 31 个通用寄存器。
/// 实际 x0(zero) 和 x2(sp) 不需要保存在这里。
#[naked]
unsafe extern "C" fn m_to_s(ctx: &mut Context) {
    core::arch::asm!(
        r"
        .altmacro
        .macro SAVE_M n
            sd x\n, \n*8(sp)
        .endm
        .macro LOAD_S n
            ld x\n, \n*8(sp)
        .endm
        ",
        // 入栈
        "
        addi sp, sp, -32*8
        ",
        // 保存 x[1..31]
        "
        .set n, 1
        .rept 31
            SAVE_M %n
            .set n, n+1
        .endr
        ",
        // M sp 保存到 S ctx
        "
        sd sp, 0(a0)
        mv sp, a0
        ",
        // 利用 tx 恢复 csr
        // S ctx.x[2](sp) => mscratch
        // S ctx.mstatus  => mstatus
        // S ctx.mepc     => mepc
        "
        ld   t0,  2*8(sp)
        ld   t1, 32*8(sp)
        ld   t2, 33*8(sp)
        csrw mscratch, t0
        csrw  mstatus, t1
        csrw     mepc, t2
        ",
        // 从 S ctx 恢复 x[1,3..32]
        "
        ld x1, 1*8(sp)
        .set n, 3
        .rept 29
            LOAD_S %n
            .set n, n+1
        .endr
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: S ctx
        "
        csrrw sp, mscratch, sp
        mret
        ",
        options(noreturn)
    )
}

/// S 态陷入 M 态。
///
/// # Safety
///
/// 裸函数。
/// 利用恢复的 ra 回到 [`m_to_s`] 的返回地址。
#[naked]
#[link_section = ".text.trap_handler"]
unsafe extern "C" fn s_to_m() {
    core::arch::asm!(
        r"
        .altmacro
        .macro SAVE_S n
            sd x\n, \n*8(sp)
        .endm
        .macro LOAD_M n
            ld x\n, \n*8(sp)
        .endm
        ",
        // 换栈：
        // sp      : S ctx
        // mscratch: S sp
        "
        csrrw sp, mscratch, sp
        ",
        // 保存 x[1,3..32] 到 S ctx
        "
        sd x1, 1*8(sp)
        .set n, 3
        .rept 29
            SAVE_S %n
            .set n, n+1
        .endr
        ",
        // 利用 tx 保存 csr
        // mscratch => S ctx.x[2](sp)
        // mstatus  => S ctx.mstatus
        // mepc     => S ctx.mepc
        "
        csrr t0, mscratch
        csrr t1, mstatus
        csrr t2, mepc
        sd   t0,  2*8(sp)
        sd   t1, 32*8(sp)
        sd   t2, 33*8(sp)
        ",
        // 从 S ctx 恢复 M sp
        "
        ld sp, 0(sp)
        ",
        // 恢复 s[0..12]
        "
        .set n, 1
        .rept 31
            LOAD_M %n
            .set n, n+1
        .endr
        ",
        // 出栈完成，栈指针归位
        // 返回
        "
        addi sp, sp, 32*8
        ret
        ",
        options(noreturn)
    )
}
