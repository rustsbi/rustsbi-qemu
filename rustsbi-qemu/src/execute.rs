use crate::{clint, hart_id, qemu_hsm::QemuHsm, Supervisor};
use core::arch::asm;
use riscv::register::*;

#[repr(usize)]
pub(crate) enum Operation {
    Stop = 0,
    SystemReset = usize::MAX,
}

pub(crate) fn execute_supervisor(hsm: &QemuHsm, supervisor: Supervisor) -> Operation {
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
        medeleg::clear_supervisor_env_call();
        medeleg::clear_machine_env_call();

        crate::set_mtvec(s_to_m as usize);
        mie::set_mext();
        mie::set_msoft();
        mie::set_mtimer();
    }

    hsm.record_current_start_finished();
    loop {
        use mcause::{Exception as E, Interrupt as I, Trap as T};

        unsafe { m_to_s(&mut ctx) };

        match mcause::read().cause() {
            T::Interrupt(I::MachineTimer) => unsafe {
                crate::clint::get().set_mtimercomp(u64::MAX);
                mip::set_stimer();
            },
            T::Interrupt(I::MachineSoft) => unsafe {
                crate::clint::get().clear_soft(hart_id());
                mip::set_ssoft();
            },
            T::Exception(E::SupervisorEnvCall) => {
                if let Some(op) = ctx.handle_ecall() {
                    return op;
                }
            }
            // TODO 可以修复非原子的非对齐访存？
            t => ctx.trap_stop(t),
        }
    }
}

#[repr(C)]
#[derive(Debug)]
struct Context {
    msp: usize,
    x: [usize; 31],
    mstatus: usize,
    mepc: usize,
}

impl Context {
    fn new(supervisor: Supervisor) -> Self {
        let mut ctx = Self {
            msp: 0,
            x: [0; 31],
            mstatus: 0,
            mepc: supervisor.start_addr,
        };

        unsafe { asm!("csrr {}, mstatus", out(reg) ctx.mstatus) };
        *ctx.a_mut(0) = hart_id();
        *ctx.a_mut(1) = supervisor.opaque;

        ctx
    }

    #[inline]
    fn x(&self, n: usize) -> usize {
        self.x[n - 1]
    }

    #[inline]
    fn x_mut(&mut self, n: usize) -> &mut usize {
        &mut self.x[n - 1]
    }

    #[inline]
    fn a(&self, n: usize) -> usize {
        self.x(n + 10)
    }

    #[inline]
    fn a_mut(&mut self, n: usize) -> &mut usize {
        self.x_mut(n + 10)
    }

    fn handle_ecall(&mut self) -> Option<Operation> {
        use rustsbi::spec::{binary::*, hsm::*, srst::*};
        let extension = self.a(7);
        let function = self.a(6);
        let ans = rustsbi::ecall(
            extension,
            function,
            [
                self.a(0),
                self.a(1),
                self.a(2),
                self.a(3),
                self.a(4),
                self.a(5),
            ],
        );
        // 判断导致退出执行流程的调用
        if ans.error == RET_SUCCESS {
            match extension {
                // 核状态
                EID_HSM => match function {
                    HART_STOP => return Some(Operation::Stop),
                    HART_SUSPEND
                        if matches!(
                            u32::try_from(self.a(0)),
                            Ok(HART_SUSPEND_TYPE_NON_RETENTIVE)
                        ) =>
                    {
                        return Some(Operation::Stop);
                    }
                    _ => {}
                },
                // 系统重置
                EID_SRST => match function {
                    SYSTEM_RESET
                        if matches!(
                            u32::try_from(self.a(0)),
                            Ok(RESET_TYPE_COLD_REBOOT) | Ok(RESET_TYPE_WARM_REBOOT)
                        ) =>
                    {
                        return Some(Operation::SystemReset)
                    }
                    _ => {}
                },

                _ => {}
            }
        }
        *self.a_mut(0) = ans.error;
        *self.a_mut(1) = ans.value;
        self.mepc = self.mepc.wrapping_add(4);
        None
    }

    #[allow(unused)]
    fn do_transfer_trap(&mut self, cause: scause::Trap) {
        unsafe {
            // 向 S 转发陷入
            mstatus::set_mpp(mstatus::MPP::Supervisor);
            // 转发陷入源状态
            let spp = match (self.mstatus >> 11) & 0b11 {
                // U
                0b00 => mstatus::SPP::User,
                // S
                0b01 => mstatus::SPP::Supervisor,
                // H/M
                mpp => unreachable!("invalid mpp: {mpp:#x} to delegate"),
            };
            mstatus::set_spp(spp);
            // 转发陷入原因
            scause::set(cause);
            // 转发陷入附加信息
            stval::write(mtval::read());
            // 转发陷入地址
            sepc::write(self.mepc);
            // 设置 S 中断状态
            if mstatus::read().sie() {
                mstatus::set_spie();
                mstatus::clear_sie();
            }
            asm!("csrr {}, mstatus", out(reg) self.mstatus);
            // 设置返回地址，返回到 S
            // TODO Vectored stvec?
            self.mepc = stvec::read().address();
        }
    }

    fn trap_stop(&self, trap: mcause::Trap) -> ! {
        println!(
            "
-----------------------------
> trap:    {trap:?}
> mstatus: {:#018x}
> mepc:    {:#018x}
> mtval:   {:#018x}
-----------------------------
",
            self.mstatus,
            self.mepc,
            mtval::read()
        );
        panic!("stopped with unsupported trap")
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
    asm!(
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
        // 利用 ctx 恢复 csr
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
    asm!(
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
        // 利用 ctx 保存 csr
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
        // 恢复 x[1..31]
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
