//! Privileged memory access module
//!
//! Reading from privileged mode memory does not need to iterate page table tree from software;
//! instead, this module makes use of `mstatus.MPRV` bit (17-th bit of `mstatus`) to read memory
//! from privileged modes under machine level.
//!
//! This module is useful when implementation need to process SBI calls with memory addresses
//! as parameters.
// Code ref: https://github.com/luojia65/zihai/blob/adb4e69ca1a4118a4de634c0682e34b67810cb0c/zihai/src/detect.rs

use core::arch::asm;
use core::mem::{self, MaybeUninit};

use riscv::register::{mcause::{Exception, Mcause, Trap}, mcause, mstatus, mtvec::{self, Mtvec, TrapMode}};

/// Pointer at supervisor level
///
/// These pointers cannot dereference directly from machine level. Instead, you may use
/// function `try_read` to get data from them.
#[derive(Debug)]
pub struct SupervisorPointer<T> {
    inner: *const T,
}

impl<T> SupervisorPointer<T> {
    /// Cast a supervisor parameter into a supervisor pointer
    ///
    /// This is a safe function for creation of a raw pointer; deref it will be unsafe.
    pub fn cast(supervisor_param: usize) -> Self {
        SupervisorPointer {
            inner: supervisor_param as *const _,
        }
    }
}

/// Reads the supervisor memory value, or fail if any exception occurred.
///
/// This function will invoke multiple instructions including reads, write, enabling
/// or disabling `mstatus.MPRV` bit. After they are executed, the value is typically returned
/// on stack or register with type `T`.
pub unsafe fn try_read<T>(src: SupervisorPointer<T>) -> Result<T, mcause::Exception> {
    let mut ans: MaybeUninit<T> = MaybeUninit::uninit();
    if mstatus::read().mprv() {
        panic!("rustsbi-qemu: mprv should be cleared before try_read")
    }
    for idx in (0..mem::size_of::<T>()).step_by(mem::size_of::<u32>()) {
        let nr = with_detect_trap(0, || asm!(
        "li     {mprv_bit}, (1 << 17)",
        "csrs   mstatus, {mprv_bit}",
        "lw     {word}, 0({in_s_addr})",
        "csrc   mstatus, {mprv_bit}",
        "sw     {word}, 0({out_m_addr})",
        mprv_bit = out(reg) _,
        word = out(reg) _,
        in_s_addr = in(reg) src.inner.cast::<u8>().add(idx),
        out_m_addr = in(reg) ans.as_mut_ptr().cast::<u8>().add(idx),
        options(nostack),
        ));
        if nr != 0 {
            return Err(Exception::from(nr))
        }
    }
    Ok(ans.assume_init())
}

// Tries to execute all instructions defined in clojure `f`.
// If resulted in an exception, this function returns its exception id.
//
// This function is useful to detect if an instruction exists on current environment.
#[inline]
fn with_detect_trap(param: usize, f: impl FnOnce()) -> usize {
    // disable interrupts and handle exceptions only
    let (mie, mtvec, tp) = unsafe { init_detect_trap(param) };
    // run detection inner
    f();
    // restore trap handler and enable interrupts
    let ans = unsafe { restore_detect_trap(mie, mtvec, tp) };
    // return the answer
    ans
}

// rust trap handler for detect exceptions
extern "C" fn rust_detect_trap(trap_frame: &mut TrapFrame) {
    // store returned exception id value into tp register
    // specially: illegal instruction => 2
    trap_frame.tp = trap_frame.mcause.bits();
    // if illegal instruction, skip current instruction
    match trap_frame.mcause.cause() {
        Trap::Exception(_) => {
            let mut insn_bits = riscv_illegal_instruction_bits((trap_frame.mtval & 0xFFFF) as u16);
            if insn_bits == 0 {
                let insn_half = unsafe { *(trap_frame.mepc as *const u16) };
                insn_bits = riscv_illegal_instruction_bits(insn_half);
            }
            // skip current instruction
            trap_frame.mepc = trap_frame.mepc.wrapping_add(insn_bits);
        }
        Trap::Interrupt(_) => unreachable!(), // filtered out for mie == false
    }
}

// Gets risc-v instruction bits from illegal instruction stval value, or 0 if unknown
#[inline]
fn riscv_illegal_instruction_bits(insn: u16) -> usize {
    if insn == 0 {
        return 0; // mtval[0..16] == 0, unknown
    }
    if insn & 0b11 != 0b11 {
        return 2; // 16-bit
    }
    if insn & 0b11100 != 0b11100 {
        return 4; // 32-bit
    }
    // FIXME: add >= 48-bit instructions in the future if we need to proceed with such instructions
    return 0; // >= 48-bit, unknown from this function by now
}

// Initialize environment for trap detection and filter in exception only
#[inline]
unsafe fn init_detect_trap(param: usize) -> (bool, Mtvec, usize) {
    // clear mie to handle exception only
    let stored_mie = mstatus::read().mie();
    mstatus::clear_mie();
    // use detect trap handler to handle exceptions
    let stored_mtvec = mtvec::read();
    let mut trap_addr = on_detect_trap as usize;
    if trap_addr & 0b1 != 0 {
        trap_addr += 0b1;
    }
    mtvec::write(trap_addr, TrapMode::Direct);
    // store tp register. tp will be used to load parameter and store return value
    let stored_tp: usize;
    asm!("mv  {}, tp", "mv  tp, {}", out(reg) stored_tp, in(reg) param, options(nomem, nostack));
    // returns preserved previous hardware states
    (stored_mie, stored_mtvec, stored_tp)
}

// Restore previous hardware states before trap detection
#[inline]
unsafe fn restore_detect_trap(mie: bool, mtvec: Mtvec, tp: usize) -> usize {
    // read the return value from tp register, and restore tp value
    let ans: usize;
    asm!("mv  {}, tp", "mv  tp, {}", out(reg) ans, in(reg) tp, options(nomem, nostack));
    // restore trap vector settings
    asm!("csrw  mtvec, {}", in(reg) mtvec.bits(), options(nomem, nostack));
    // enable interrupts
    if mie {
        mstatus::set_mie();
    };
    ans
}

// Trap frame for instruction exception detection
#[repr(C)]
struct TrapFrame {
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
    mstatus: usize,
    mepc: usize,
    mcause: Mcause,
    mtval: usize,
}

// Assembly trap handler for instruction detection.
//
// This trap handler shares the same stack from its prospective caller,
// the caller must ensure it has abundant stack size for a trap handler.
//
// This function should not be used in conventional trap handling,
// as it does not preserve a special trap stack, and it's designed to
// handle exceptions only rather than interrupts.
#[naked]
unsafe extern "C" fn on_detect_trap() -> ! {
    asm!(
    ".p2align 2",
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
    "li     t4, (1 << 17)", // clear mstatus.mprv
    "csrc   mstatus, t4",
    "call   {rust_detect_trap}",
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
    "sret",
    rust_detect_trap = sym rust_detect_trap,
    options(noreturn),
    )
}
