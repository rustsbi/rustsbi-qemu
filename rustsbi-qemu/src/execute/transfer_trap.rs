use super::Context;
use riscv::register::{mstatus, mtval, scause, sepc, stval, stvec};

pub(super) fn should_transfer_trap(ctx: &Context) -> bool {
    (ctx.mstatus >> 11) & 0b11 != 0b11
}

pub(super) fn do_transfer_trap(ctx: &mut Context, cause: scause::Trap) {
    unsafe {
        // 填写陷入原因
        scause::set(cause);
        // 填写陷入附加信息
        stval::write(mtval::read());
        // 填写 S 态层需要返回到的地址
        sepc::write(ctx.mepc);
        // 设置中断位
        mstatus::set_mpp(mstatus::MPP::Supervisor);
        mstatus::set_spp(mstatus::SPP::Supervisor);
        if mstatus::read().sie() {
            mstatus::set_spie()
        }
        mstatus::clear_sie();
        core::arch::asm!("csrr {}, mstatus", out(reg) ctx.mstatus);
        // 设置返回地址，返回到S层
        // TODO Vectored stvec?
        ctx.mepc = stvec::read().address();
    }
}
