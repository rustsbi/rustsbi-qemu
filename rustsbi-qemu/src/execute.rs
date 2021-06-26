use crate::feature;
use crate::runtime::{MachineTrap, Runtime, SupervisorContext};
use core::{
    ops::{Generator, GeneratorState},
    pin::Pin,
};
use riscv::register::{mie, mip};

pub fn execute_supervisor(supervisor_mepc: usize, a0: usize, a1: usize) -> ! {
    let mut rt = Runtime::new_sbi_supervisor(supervisor_mepc, a0, a1);
    loop {
        match Pin::new(&mut rt).resume(()) {
            GeneratorState::Yielded(MachineTrap::SbiCall()) => {
                let ctx = rt.context_mut();
                let param = [ctx.a0, ctx.a1, ctx.a2, ctx.a3, ctx.a4];
                let ans = rustsbi::ecall(ctx.a7, ctx.a6, param);
                ctx.a0 = ans.error;
                ctx.a1 = ans.value;
                ctx.mepc = ctx.mepc.wrapping_add(4);
            }
            GeneratorState::Yielded(MachineTrap::IllegalInstruction()) => {
                let ctx = rt.context_mut();
                // FIXME: get_vaddr_u32这个过程可能出错。
                let ins = unsafe { get_vaddr_u32(ctx.mepc) } as usize;
                if !emulate_illegal_instruction(ctx, ins) {
                    unsafe {
                        if should_transfer_illegal_instruction(ctx) {
                            do_transfer_illegal_instruction(ctx)
                        } else {
                            fail_illegal_instruction(ctx, ins)
                        }
                    }
                }
            }
            GeneratorState::Yielded(MachineTrap::MachineTimer()) => unsafe {
                mip::set_stimer();
                mie::clear_mtimer();
            },
            GeneratorState::Complete(()) => {
                use rustsbi::Reset;
                crate::test_device::Reset.system_reset(
                    rustsbi::reset::RESET_TYPE_SHUTDOWN,
                    rustsbi::reset::RESET_REASON_NO_REASON,
                );
            }
        }
    }
}

#[inline]
unsafe fn get_vaddr_u32(vaddr: usize) -> u32 {
    let mut ans: u32;
    asm!("
        li      {tmp}, (1 << 17)
        csrrs   {tmp}, mstatus, {tmp}
        lwu     {ans}, 0({vaddr})
        csrw    mstatus, {tmp}
        ",
        tmp = out(reg) _,
        vaddr = in(reg) vaddr,
        ans = lateout(reg) ans
    );
    ans
}

fn emulate_illegal_instruction(ctx: &mut SupervisorContext, ins: usize) -> bool {
    if feature::emulate_rdtime(ctx, ins) {
        return true;
    }
    false
}

unsafe fn should_transfer_illegal_instruction(ctx: &mut SupervisorContext) -> bool {
    use riscv::register::mstatus::MPP;
    ctx.mstatus.mpp() != MPP::Machine
}

unsafe fn do_transfer_illegal_instruction(ctx: &mut SupervisorContext) {
    use riscv::register::{
        mstatus::{self, MPP, SPP},
        mtval, scause, sepc, stval, stvec,
    };
    // 设置S层异常原因为：非法指令
    scause::set(scause::Trap::Exception(
        scause::Exception::IllegalInstruction,
    ));
    // 填写异常指令的指令内容
    stval::write(mtval::read());
    // 填写S层需要返回到的地址，这里的mepc会被随后的代码覆盖掉
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

// 真·非法指令异常，是M层出现的
fn fail_illegal_instruction(ctx: &mut SupervisorContext, ins: usize) -> ! {
    #[cfg(target_pointer_width = "64")]
    panic!("invalid instruction from machine level, mepc: {:016x?}, instruction: {:016x?}, context: {:016x?}", ctx.mepc, ins, ctx);
    #[cfg(target_pointer_width = "32")]
    panic!("invalid instruction from machine level, mepc: {:08x?}, instruction: {:08x?}, context: {:08x?}", ctx.mepc, ins, ctx);
}
