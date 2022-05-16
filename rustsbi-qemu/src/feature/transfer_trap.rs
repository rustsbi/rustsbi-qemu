use crate::runtime::SupervisorContext;
use riscv::register::{
    mstatus::{self, MPP, SPP},
    mtval, scause, sepc, stval, stvec,
};

#[inline]
pub unsafe fn should_transfer_trap(ctx: &mut SupervisorContext) -> bool {
    ctx.mstatus.mpp() != MPP::Machine
}

#[inline]
pub unsafe fn do_transfer_trap(ctx: &mut SupervisorContext, cause: scause::Trap) {
    // 设置S层异常原因为：非法指令
    scause::set(cause);
    // 填写异常指令的指令内容
    stval::write(mtval::read());
    // 填写S层需要返回到的地址，这里的mepc会被随后的代码覆盖掉。mepc已经处理了中断向量的问题
    sepc::write(ctx.mepc);
    // 设置中断位
    mstatus::set_mpp(MPP::Supervisor);
    mstatus::set_spp(SPP::Supervisor);
    if mstatus::read().sie() {
        mstatus::set_spie()
    }
    mstatus::clear_sie();
    ctx.mstatus = mstatus::read();
    // 设置返回地址，返回到S层
    // 注意，无论是Direct还是Vectored模式，所有异常的向量偏移都是0，不需要处理中断向量，跳转到入口地址即可
    ctx.mepc = stvec::read().address();
}
