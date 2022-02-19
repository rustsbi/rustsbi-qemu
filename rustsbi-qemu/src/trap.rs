// use trap based interrupt handler instead of generator based by now

use core::arch::asm;
use riscv::register::mcause::Mcause;
use riscv::register::mstatus::Mstatus;

extern "C" fn rust_trap_exception(_trap_frame: &mut TrapFrame) {
    todo!("trap exception")
}

extern "C" fn rust_machine_software(_trap_frame: &mut TrapFrame) {
    todo!("machine software")
}

extern "C" fn rust_machine_timer(_trap_frame: &mut TrapFrame) {
    todo!("machine timer")
}

extern "C" fn rust_machine_external(_trap_frame: &mut TrapFrame) {
    todo!("machine external")
}

extern "C" fn rust_interrupt_reserved(_trap_frame: &mut TrapFrame) {
    panic!("entered handler for reserved interrupt")
}

#[derive(Debug)]
#[repr(C)]
pub struct TrapFrame {
    ra: usize,
    tp: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
    mstatus: Mstatus,
    mepc: usize,
    mcause: Mcause,
    mtval: usize,
}

#[naked]
#[link_section = ".text"]
unsafe extern "C" fn trap_vector() {
    asm!(
    ".option push
	.option norvc",
    "j	{trap_exception}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{machine_software}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{machine_timer}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{machine_external}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    ".option pop",
    trap_exception = sym trap_exception,
    machine_software = sym machine_software,
    machine_timer = sym machine_timer,
    machine_external = sym machine_external,
    interrupt_reserved = sym interrupt_reserved,
    options(noreturn))
}

macro_rules! decl_trap_handler {
    ($(($trap: ident, $handler: ident),)+) => {
        $(
#[naked]
#[link_section = ".text"]
unsafe extern "C" fn $trap() -> ! {
    asm!(
    "csrrw  sp, mscratch, sp",
    "addi   sp, sp, -8*21",
    "sd     ra, 0*8(sp)",
    "sd     tp, 1*8(sp)",
    "sd     a0, 2*8(sp)",
    "sd     a1, 3*8(sp)",
    "sd     a2, 4*8(sp)",
    "sd     a3, 5*8(sp)",
    "sd     a4, 6*8(sp)",
    "sd     a5, 7*8(sp)",
    "sd     a6, 8*8(sp)",
    "sd     a7, 9*8(sp)",
    "sd     t0, 10*8(sp)",
    "sd     t1, 11*8(sp)",
    "sd     t2, 12*8(sp)",
    "sd     t3, 13*8(sp)",
    "sd     t4, 14*8(sp)",
    "sd     t5, 15*8(sp)",
    "sd     t6, 16*8(sp)",
    "csrr   t0, mstatus",
    "sd     t0, 17*8(sp)",
    "csrr   t1, mepc",
    "sd     t1, 18*8(sp)",
    "csrr   t2, mcause",
    "sd     t2, 19*8(sp)",
    "csrr   t3, mtval",
    "sd     t3, 20*8(sp)",
    "mv     a0, sp",
    "call   {}",
    "ld     t0, 17*8(sp)",
    "csrw   mstatus, t0",
    "ld     t1, 18*8(sp)",
    "csrw   mepc, t1",
    "ld     t2, 19*8(sp)",
    "csrw   mcause, t2",
    "ld     t3, 20*8(sp)",
    "csrw   mtval, t3",
    "ld     ra, 0*8(sp)",
    "ld     tp, 1*8(sp)",
    "ld     a0, 2*8(sp)",
    "ld     a1, 3*8(sp)",
    "ld     a2, 4*8(sp)",
    "ld     a3, 5*8(sp)",
    "ld     a4, 6*8(sp)",
    "ld     a5, 7*8(sp)",
    "ld     a6, 8*8(sp)",
    "ld     a7, 9*8(sp)",
    "ld     t0, 10*8(sp)",
    "ld     t1, 11*8(sp)",
    "ld     t2, 12*8(sp)",
    "ld     t3, 13*8(sp)",
    "ld     t4, 14*8(sp)",
    "ld     t5, 15*8(sp)",
    "ld     t6, 16*8(sp)",
    "addi   sp, sp, 8*21",
    "csrrw  sp, mscratch, sp",
    "mret",
    sym $handler,
    options(noreturn)
    )
}
        )+
    };
}

decl_trap_handler! {
    (trap_exception, rust_trap_exception),
    (machine_software, rust_machine_software),
    (machine_timer, rust_machine_timer),
    (machine_external, rust_machine_external),
    (interrupt_reserved, rust_interrupt_reserved),
}
