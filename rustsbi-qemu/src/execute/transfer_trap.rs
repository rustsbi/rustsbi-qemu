use super::Context;
use riscv::register::{mstatus, mtval, scause, sepc, stval, stvec};

pub(super) fn should_transfer_trap(ctx: &Context) -> bool {
    (ctx.mstatus >> 11) & 0b11 != 0b11
}

pub(super) fn do_transfer_trap(ctx: &mut Context, cause: scause::Trap) {
    unsafe {
        // 向 S 转发陷入
        mstatus::set_mpp(mstatus::MPP::Supervisor);
        // 转发陷入源状态
        let spp = match (ctx.mstatus >> 11) & 0b11 {
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
        sepc::write(ctx.mepc);
        // 设置 S 中断状态
        if mstatus::read().sie() {
            mstatus::set_spie();
            mstatus::clear_sie();
        }
        core::arch::asm!("csrr {}, mstatus", out(reg) ctx.mstatus);
        // 设置返回地址，返回到 S
        // TODO Vectored stvec?
        ctx.mepc = stvec::read().address();
    }
}
