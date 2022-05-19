use crate::{
    hart_id,
    qemu_hsm::{QemuHsm, EID_HSM, FID_HART_STOP, FID_HART_SUSPEND, SUSPEND_NON_RETENTIVE},
};
use core::{
    ops::{Generator, GeneratorState},
    pin::Pin,
};
use riscv::register::{
    mcause,
    scause::{Exception, Interrupt, Trap},
};

mod feature;
mod prv_mem;
mod runtime;

use prv_mem::SupervisorPointer;
use runtime::{MachineTrap, Runtime, SupervisorContext};

pub(crate) fn execute_supervisor(hsm: &'static QemuHsm) {
    let mut rt = if let Some(supervisor) = hsm.take_supervisor() {
        Runtime::new(supervisor)
    } else {
        return;
    };

    hsm.record_current_start_finished();
    loop {
        match Pin::new(&mut rt).resume(()) {
            GeneratorState::Yielded(MachineTrap::SbiCall()) => {
                let ctx = rt.context_mut();
                let param = [ctx.a0, ctx.a1, ctx.a2, ctx.a3, ctx.a4, ctx.a5];
                let ans = rustsbi::ecall(ctx.a7, ctx.a6, param);
                if ctx.a7 == EID_HSM && ans.error == 0 {
                    if ctx.a6 == FID_HART_STOP {
                        return;
                    }
                    if ctx.a6 == FID_HART_SUSPEND && ctx.a0 == SUSPEND_NON_RETENTIVE as usize {
                        return;
                    }
                }
                ctx.a0 = ans.error;
                ctx.a1 = ans.value;
                ctx.mepc = ctx.mepc.wrapping_add(4);
            }
            GeneratorState::Yielded(MachineTrap::IllegalInstruction()) => {
                let ctx = rt.context_mut();
                let ptr: SupervisorPointer<usize> = SupervisorPointer::cast(ctx.mepc);
                let deref_ans = unsafe { prv_mem::try_read(ptr) };
                let ins = match deref_ans {
                    Ok(ins) => ins,
                    Err(e) => fail_cant_read_exception_address(ctx, e),
                };
                if !emulate_illegal_instruction(ctx, ins) {
                    unsafe {
                        if feature::should_transfer_trap(ctx) {
                            feature::do_transfer_trap(
                                ctx,
                                Trap::Exception(Exception::IllegalInstruction),
                            )
                        } else {
                            fail_illegal_instruction(ctx, ins)
                        }
                    }
                }
            }
            GeneratorState::Yielded(MachineTrap::MachineTimer()) => unsafe {
                use riscv::register::{mie, mip};
                mip::set_stimer();
                mie::clear_mtimer();
            },
            GeneratorState::Yielded(MachineTrap::MachineSoft()) => {
                // machine software interrupt but no HSM commands - delegate to S mode;
                let ctx = rt.context_mut();
                crate::clint::get().clear_soft(hart_id()); // Clear IPI
                unsafe {
                    if feature::should_transfer_trap(ctx) {
                        feature::do_transfer_trap(ctx, Trap::Interrupt(Interrupt::SupervisorSoft));
                    } else {
                        panic!("rustsbi-qemu: machine soft interrupt with no hart state monitor command");
                    }
                }
            }
            GeneratorState::Complete(()) => {
                use rustsbi::{
                    reset::{RESET_REASON_NO_REASON, RESET_TYPE_SHUTDOWN},
                    Reset,
                };
                crate::test_device::get().system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
            }
        }
    }
}

#[inline]
fn emulate_illegal_instruction(ctx: &mut SupervisorContext, ins: usize) -> bool {
    if feature::emulate_rdtime(ctx, ins) {
        return true;
    }
    false
}

// Illegal instruction occurred in M level
fn fail_illegal_instruction(ctx: &mut SupervisorContext, ins: usize) -> ! {
    #[cfg(target_pointer_width = "64")]
    panic!("invalid instruction from machine level, mepc: {:016x?}, instruction: {:016x?}, context: {:016x?}", ctx.mepc, ins, ctx);
    #[cfg(target_pointer_width = "32")]
    panic!("invalid instruction from machine level, mepc: {:08x?}, instruction: {:08x?}, context: {:08x?}", ctx.mepc, ins, ctx);
}

fn fail_cant_read_exception_address(ctx: &mut SupervisorContext, cause: mcause::Exception) -> ! {
    #[cfg(target_pointer_width = "64")]
    panic!(
        "can't read exception address, cause: {:?}, mepc: {:016x?}, context: {:016x?}",
        cause, ctx.mepc, ctx
    );
    #[cfg(target_pointer_width = "32")]
    panic!(
        "can't read exception address, cause: {:?}, mepc: {:08x?}, context: {:08x?}",
        cause, ctx.mepc, ctx
    );
}
