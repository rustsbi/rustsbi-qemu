use core::{
    ops::{Generator, GeneratorState},
    pin::Pin,
};

use riscv::register::{mcause, mie, mip, scause::{Exception, Trap}};
use riscv::register::scause::Interrupt;

use crate::feature;
use crate::prv_mem::{self, SupervisorPointer};
use crate::qemu_hsm::{HsmCommand, pause, QemuHsm};
use crate::runtime::{MachineTrap, Runtime, SupervisorContext};

pub fn execute_supervisor(supervisor_mepc: usize, hart_id: usize, a1: usize, hsm: QemuHsm) -> ! {
    let mut rt = Runtime::new_sbi_supervisor(supervisor_mepc, hart_id, a1);
    hsm.record_current_start_finished();
    loop {
        match Pin::new(&mut rt).resume(()) {
            GeneratorState::Yielded(MachineTrap::SbiCall()) => {
                let ctx = rt.context_mut();
                let param = [ctx.a0, ctx.a1, ctx.a2, ctx.a3, ctx.a4, ctx.a5];
                let ans = rustsbi::ecall(ctx.a7, ctx.a6, param);
                if ans.error == 0x233 {
                    // hart non-retentive resume
                    if let Some(HsmCommand::Start(start_paddr, opaque)) = hsm.last_command() {
                        unsafe {
                            riscv::register::satp::write(0);
                            riscv::register::sstatus::clear_sie();
                        }
                        hsm.record_current_start_finished();
                        ctx.mstatus = riscv::register::mstatus::read(); // get from modified sstatus
                        ctx.a0 = hart_id;
                        ctx.a1 = opaque;
                        ctx.mepc = start_paddr;
                    }
                } else {
                    ctx.a0 = ans.error;
                    ctx.a1 = ans.value;
                    ctx.mepc = ctx.mepc.wrapping_add(4);
                }
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
                mip::set_stimer();
                mie::clear_mtimer();
            },
            GeneratorState::Yielded(MachineTrap::MachineSoft()) => match hsm.last_command() {
                Some(HsmCommand::Start(_start_paddr, _opaque)) => {
                    panic!("rustsbi-qemu: illegal state")
                }
                Some(HsmCommand::Stop) => {
                    // no hart stop command in qemu, record stop state and pause
                    hsm.record_current_stop_finished();
                    pause();
                    if let Some(HsmCommand::Start(start_paddr, opaque)) = hsm.last_command() {
                        // Resuming from a non-retentive suspend state is relatively more involved and requires software
                        // to restore various hart registers and CSRs for all privilege modes.
                        // Upon resuming from non-retentive suspend state, the hart will jump to supervisor-mode at address
                        // specified by `resume_addr` with specific registers values described in the table below:
                        //
                        // | Register Name | Register Value
                        // |:--------------|:--------------
                        // | `satp`        | 0
                        // | `sstatus.SIE` | 0
                        // | a0            | hartid
                        // | a1            | `opaque` parameter
                        unsafe {
                            riscv::register::satp::write(0);
                            riscv::register::sstatus::clear_sie();
                        }
                        hsm.record_current_start_finished();
                        let ctx = rt.context_mut();
                        ctx.mstatus = riscv::register::mstatus::read(); // get from modified sstatus
                        ctx.a0 = hart_id;
                        ctx.a1 = opaque;
                        ctx.mepc = start_paddr;
                    }
                }
                None => unsafe {
                    // machine software interrupt but no HSM commands - delegate to S mode;
                    let ctx = rt.context_mut();
                    let clint = crate::clint::Clint::new(0x2000000 as *mut u8);
                    clint.clear_soft(hart_id); // Clear IPI
                    if feature::should_transfer_trap(ctx) {
                        feature::do_transfer_trap(
                            ctx,
                            Trap::Interrupt(Interrupt::SupervisorSoft),
                        )
                    } else {
                        panic!("rustsbi-qemu: machine soft interrupt with no hart state monitor command")
                    }
                },
            },
            GeneratorState::Complete(()) => {
                use rustsbi::Reset;
                crate::test_device::SiFiveTest.system_reset(
                    rustsbi::reset::RESET_TYPE_SHUTDOWN,
                    rustsbi::reset::RESET_REASON_NO_REASON,
                );
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
    panic!("can't read exception address, cause: {:?}, mepc: {:016x?}, context: {:016x?}", cause, ctx.mepc, ctx);
    #[cfg(target_pointer_width = "32")]
    panic!("can't read exception address, cause: {:?}, mepc: {:08x?}, context: {:08x?}", cause, ctx.mepc, ctx);
}
