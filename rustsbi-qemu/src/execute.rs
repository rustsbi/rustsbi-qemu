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

// #[repr(usize)]
// pub(crate) enum Operation {
//     Stop = 0,
//     SystemReset = 1,
// }

// pub(crate) fn execute_supervisor(
//     sbi: FixedRustSBI,
//     hsm: &QemuHsm,
//     supervisor: Supervisor,
// ) -> Operation {
//     clint::msip::clear();
//     clint::mtimecmp::clear();
//     unsafe {
//         asm!("csrw mideleg, {}", in(reg) !0);
//         asm!("csrw medeleg, {}", in(reg) !0);
//         asm!("csrw mcounteren, {}", in(reg) !0);
//         medeleg::clear_supervisor_env_call();
//         medeleg::clear_machine_env_call();

//         mtvec::write(trap_vec as _, mtvec::TrapMode::Vectored);
//         mie::set_mext();
//         mie::set_msoft();
//         mie::set_mtimer();
//     }
//     let ctx = SupervisorContext::new(supervisor);

//     let mut env = Environment { ctx, sbi };
//     mscratch::write(&mut env.ctx as *mut _ as _);

//     hsm.record_current_start_finished();
//     loop {
//         unsafe { m_to_s() };

//         use mcause::{Exception, Trap};
//         match mcause::read().cause() {
//             Trap::Exception(Exception::SupervisorEnvCall) => {
//                 if let Some(op) = env.handle_ecall() {
//                     break op;
//                 }
//             }
//             t => env.trap_stop(t),
//         }
//     }
// }

// struct Environment<'a> {
//     ctx: SupervisorContext,
//     sbi: FixedRustSBI<'a>,
// }

// impl<'a> Environment<'a> {
//     fn handle_ecall(&mut self) -> Option<Operation> {
//         use rustsbi::spec::{binary::*, hsm::*, srst::*};
//         if self.ctx.sbi_extension() == sbi_spec::legacy::LEGACY_CONSOLE_PUTCHAR {
//             let ch = self.ctx.a(0);
//             print!("{:}", ch as u8 as char);
//             self.ctx.mepc = self.ctx.mepc.wrapping_add(4);
//             return None;
//         } else if self.ctx.sbi_extension() == sbi_spec::legacy::LEGACY_CONSOLE_GETCHAR {
//             *self.ctx.a_mut(0) = unsafe { crate::UART.lock().assume_init_mut().receive() } as usize;
//             self.ctx.mepc = self.ctx.mepc.wrapping_add(4);
//             return None;
//         }
//         let ans = self.sbi.handle_ecall(
//             self.ctx.sbi_extension(),
//             self.ctx.sbi_function(),
//             self.ctx.sbi_param(),
//         );
//         // 判断导致退出执行流程的调用
//         if ans.error == RET_SUCCESS {
//             match (self.ctx.sbi_extension(), self.ctx.sbi_function()) {
//                 // 核状态
//                 (EID_HSM, HART_STOP) => return Some(Operation::Stop),
//                 (EID_HSM, HART_SUSPEND)
//                     if matches!(
//                         u32::try_from(self.ctx.a(0)),
//                         Ok(HART_SUSPEND_TYPE_NON_RETENTIVE)
//                     ) =>
//                 {
//                     return Some(Operation::Stop)
//                 }
//                 // 系统重置
//                 (EID_SRST, SYSTEM_RESET)
//                     if matches!(
//                         u32::try_from(self.ctx.a(0)),
//                         Ok(RESET_TYPE_COLD_REBOOT) | Ok(RESET_TYPE_WARM_REBOOT)
//                     ) =>
//                 {
//                     return Some(Operation::SystemReset)
//                 }
//                 _ => {}
//             }
//         }
//         self.ctx.fill_in(ans);
//         self.ctx.mepc = self.ctx.mepc.wrapping_add(4);
//         None
//     }

//     fn trap_stop(&self, trap: mcause::Trap) -> ! {
//         println!(
//             "
// -----------------------------
// > trap:    {trap:?}
// > mstatus: {:#018x}
// > mepc:    {:#018x}
// > mtval:   {:#018x}
// -----------------------------
// ",
//             self.ctx.mstatus,
//             self.ctx.mepc,
//             mtval::read()
//         );
//         panic!("stopped with unsupported trap")
//     }
// }

// #[repr(C)]
// #[derive(Debug)]
// struct SupervisorContext {
//     msp: usize,
//     x: [usize; 31],
//     mstatus: usize,
//     mepc: usize,
// }

// impl SupervisorContext {
//     fn new(supervisor: Supervisor) -> Self {
//         let mut ctx = Self {
//             msp: 0,
//             x: [0; 31],
//             mstatus: 0,
//             mepc: supervisor.start_addr,
//         };

//         unsafe {
//             mstatus::set_mpp(mstatus::MPP::Supervisor);
//             mstatus::set_mpie();
//             asm!("csrr {}, mstatus", out(reg) ctx.mstatus)
//         };
//         *ctx.a_mut(0) = hart_id();
//         *ctx.a_mut(1) = supervisor.opaque;

//         ctx
//     }

//     #[inline]
//     fn sbi_extension(&self) -> usize {
//         self.a(7)
//     }

//     #[inline]
//     fn sbi_function(&self) -> usize {
//         self.a(6)
//     }

//     #[inline]
//     fn sbi_param(&self) -> [usize; 6] {
//         [
//             self.a(0),
//             self.a(1),
//             self.a(2),
//             self.a(3),
//             self.a(4),
//             self.a(5),
//         ]
//     }

//     #[inline]
//     fn fill_in(&mut self, ans: SbiRet) {
//         *self.a_mut(0) = ans.error;
//         *self.a_mut(1) = ans.value;
//     }

//     #[inline]
//     fn x(&self, n: usize) -> usize {
//         self.x[n - 1]
//     }

//     #[inline]
//     fn x_mut(&mut self, n: usize) -> &mut usize {
//         &mut self.x[n - 1]
//     }

//     #[inline]
//     fn a(&self, n: usize) -> usize {
//         self.x(n + 10)
//     }

//     #[inline]
//     fn a_mut(&mut self, n: usize) -> &mut usize {
//         self.x_mut(n + 10)
//     }

//     #[allow(unused)]
//     fn do_transfer_trap(&mut self, cause: scause::Trap) {
//         unsafe {
//             // 向 S 转发陷入
//             mstatus::set_mpp(mstatus::MPP::Supervisor);
//             // 转发陷入源状态
//             let spp = match (self.mstatus >> 11) & 0b11 {
//                 // U
//                 0b00 => mstatus::SPP::User,
//                 // S
//                 0b01 => mstatus::SPP::Supervisor,
//                 // H/M
//                 mpp => unreachable!("invalid mpp: {mpp:#x} to delegate"),
//             };
//             mstatus::set_spp(spp);
//             // 转发陷入原因
//             scause::set(cause);
//             // 转发陷入附加信息
//             stval::write(mtval::read());
//             // 转发陷入地址
//             sepc::write(self.mepc);
//             // 设置 S 中断状态
//             if mstatus::read().sie() {
//                 mstatus::set_spie();
//                 mstatus::clear_sie();
//             }
//             asm!("csrr {}, mstatus", out(reg) self.mstatus);
//             // 设置返回地址，返回到 S
//             // TODO Vectored stvec?
//             self.mepc = stvec::read().address();
//         }
//     }
// }

// /// M 态转到 S 态。
// ///
// /// # Safety
// ///
// /// 裸函数，手动保存所有上下文环境。
// /// 为了写起来简单，占 32 * usize 空间，循环 31 次保存 31 个通用寄存器。
// /// 实际 x0(zero) 和 x2(sp) 不需要保存在这里。
// #[naked]
// unsafe extern "C" fn m_to_s() {
//     asm!(
//         r"  .altmacro
//             .macro SAVE_M n
//                 sd x\n, \n*8(sp)
//             .endm
//             .macro LOAD_S n
//                 ld x\n, \n*8(sp)
//             .endm
//         ",
//         // 初始化栈帧：sp = Mctx
//         "   addi sp, sp, -32*8",
//         // 特权上下文地址保存到机器上下文
//         "   csrr  t0, mscratch
//             sd    t0, (sp)
//         ",
//         // 保存机器上下文
//         "   .set n, 1
//             .rept 31
//                 SAVE_M %n
//                 .set n, n+1
//             .endr
//         ",
//         // 切换上下文：sp = Sctx
//         "   csrrw sp, mscratch, sp",
//         // 机器上下文地址保存到特权上下文
//         "   csrr  t0, mscratch
//             sd    t0, (sp)
//         ",
//         // 恢复 csr
//         "   ld   t0, 32*8(sp)
//             ld   t1, 33*8(sp)
//             csrw mstatus, t0
//             csrw    mepc, t1
//         ",
//         // 恢复特权上下文
//         "   ld x1, 1*8(sp)
//             .set n, 3
//             .rept 29
//                 LOAD_S %n
//                 .set n, n+1
//             .endr
//             ld sp, 2*8(sp)
//         ",
//         // 执行特权程序
//         "   mret",
//         options(noreturn)
//     )
// }

// /// S 态陷入 M 态。
// ///
// /// # Safety
// ///
// /// 裸函数。
// /// 利用恢复的 ra 回到 [`m_to_s`] 的返回地址。
// #[naked]
// unsafe extern "C" fn s_to_m() {
//     asm!(
//         r"
//         .altmacro
//         .macro SAVE_S n
//             sd x\n, \n*8(sp)
//         .endm
//         .macro LOAD_M n
//             ld x\n, \n*8(sp)
//         .endm
//         ",
//         // 切换上下文：sp = Sctx
//         "   csrrw sp, mscratch, sp
//             ld    sp, (sp)
//         ",
//         // 保存特权上下文
//         "   sd x1, 1*8(sp)
//             .set n, 3
//             .rept 29
//                 SAVE_S %n
//                 .set n, n+1
//             .endr
//             csrrw t0, mscratch, sp
//             sd    t0,  2*8(sp)
//         ",
//         // 保存 csr
//         "   csrr t1, mstatus
//             csrr t2, mepc
//             sd   t1, 32*8(sp)
//             sd   t2, 33*8(sp)
//         ",
//         // 切换上下文：sp = Mctx
//         "   ld sp, (sp)",
//         // 恢复机器上下文
//         "   .set n, 1
//             .rept 31
//                 LOAD_M %n
//                 .set n, n+1
//             .endr
//         ",
//         // 栈帧释放，返回
//         "   addi sp, sp, 32*8
//             ret
//         ",
//         options(noreturn)
//     )
// }
