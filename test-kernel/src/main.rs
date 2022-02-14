// A test kernel to test RustSBI function on all platforms
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

use riscv::register::{scause::{self, Exception, Trap}, sepc, /*sie, sstatus, */stvec::{self, TrapMode}};
use riscv::register::scause::Interrupt;

#[macro_use]
mod console;
mod mm;
mod sbi;

pub extern "C" fn rust_main(hartid: usize, dtb_pa: usize) -> ! {
    unsafe { asm!("mv tp, {}", in(reg) hartid, options(nomem, nostack)) }; // tp == hartid
    let mut start_trap_addr = start_trap as usize;
    if start_trap_addr & 0b10 != 0 {
        start_trap_addr += 0b10;
    }
    if hartid == 0 {
        // initialization
        mm::init_heap();
    }
    if hartid == 0 {
        println!(
            "<< Test-kernel: Hart id = {}, DTB physical address = {:#x}",
            hartid, dtb_pa
        );
        test_base_extension();
        test_sbi_ins_emulation();
        unsafe { stvec::write(start_trap_addr, TrapMode::Direct) };
        println!(">> Test-kernel: Trigger illegal exception");
        unsafe { asm!("csrw mcycle, x0") }; // mcycle cannot be written, this is always a 4-byte illegal instruction
    }
    if hartid == 0 {
        let sbi_ret = sbi::hart_stop(3);
        println!(">> Stop hart 3, return value {:?}", sbi_ret);
        for i in 0..5 {
            let sbi_ret = sbi::hart_get_status(i);
            println!(">> Hart {} state return value: {:?}", i, sbi_ret);
        }
    } else if hartid == 1 {
        let sbi_ret = sbi::hart_suspend(0x00000000, 0, 0);
        println!(
            ">> Start test for hart {}, retentive suspend return value {:?}",
            hartid, sbi_ret
        );
    } else if hartid == 2 {
        /* resume_addr should be physical address, and here pa == va */
        let sbi_ret = sbi::hart_suspend(0x80000000, hart_2_resume as usize, 0x4567890a);
        println!(">> Error for non-retentive suspend: {:?}", sbi_ret);
        loop {}
    } else if hartid == 4 {
        // unsafe { stvec::write(start_trap_addr, TrapMode::Direct) };
        // unsafe { sstatus::set_sie() };
        // unsafe { sie::set_ssoft() };
        // loop {} // wait for S-IPI
        // println!(">> Test-kernel: SBI S-IPI delegation success");
        // println!("<< Test-kernel: All hart SBI test SUCCESS, shutdown");
        loop {} // todo: S-IPI
    } else {
        // hartid == 3
        loop {}
    }
    if hartid == 0 {
        println!(
            "<< Test-kernel: test for hart {} success, wake another hart",
            hartid
        );
        let sbi_ret = sbi::send_ipi(0b10, 0); // wake hart 1
        println!(">> Wake hart 1, sbi return value {:?}", sbi_ret);
        loop {} // wait for machine shutdown
    } else if hartid == 1 {
        // send software IPI to activate hart 2
        let sbi_ret = sbi::send_ipi(0b1, 2);
        println!(">> Wake hart 2, sbi return value {:?}", sbi_ret);
        loop {}
    } else {
        // hartid == 2 || hartid == 3 || hartid == 4
        unreachable!()
    }
}

extern "C" fn hart_2_resume(hart_id: usize, param: usize) {
    println!(
        "<< The parameter passed to hart {} resume is: {:#x}",
        hart_id, param
    );
    let param = 0x12345678;
    println!(">> Start hart 3 with parameter {:#x}", param);
    /* start_addr should be physical address, and here pa == va */
    let sbi_ret = sbi::hart_start(3, hart_3_start as usize, param);
    println!(">> SBI return value: {:?}", sbi_ret);
    loop {} // wait for machine shutdown
}

extern "C" fn hart_3_start(hart_id: usize, param: usize) {
    println!(
        "<< The parameter passed to hart {} start is: {:#x}",
        hart_id, param
    );
    println!("<< Test-kernel: All hart SBI test SUCCESS, shutdown");
    sbi::shutdown()
    // todo: S-IPI
    // println!(">> Send IPI to hart 4, should delegate IPI to S-level");
    // let _ = sbi::send_ipi(0b1, 4); // IPI to hart 4
    // loop {} // wait for machine shutdown
}

fn test_base_extension() {
    println!(">> Test-kernel: Testing base extension");
    let base_version = sbi::probe_extension(sbi::EXTENSION_BASE);
    if base_version == 0 {
        println!("!! Test-kernel: no base extension probed; SBI call returned value '0'");
        println!(
            "!! Test-kernel: This SBI implementation may only have legacy extension implemented"
        );
        println!("!! Test-kernel: SBI test FAILED due to no base extension found");
        sbi::shutdown()
    }
    println!("<< Test-kernel: Base extension version: {:x}", base_version);
    println!(
        "<< Test-kernel: SBI specification version: {:x}",
        sbi::get_spec_version()
    );
    println!(
        "<< Test-kernel: SBI implementation Id: {:x}",
        sbi::get_sbi_impl_id()
    );
    println!(
        "<< Test-kernel: SBI implementation version: {:x}",
        sbi::get_sbi_impl_version()
    );
    println!(
        "<< Test-kernel: Device mvendorid: {:x}",
        sbi::get_mvendorid()
    );
    println!("<< Test-kernel: Device marchid: {:x}", sbi::get_marchid());
    println!("<< Test-kernel: Device mimpid: {:x}", sbi::get_mimpid());
}

fn test_sbi_ins_emulation() {
    println!(">> Test-kernel: Testing SBI instruction emulation");
    let time_start = riscv::register::time::read64();
    println!("<< Test-kernel: Current time: {:x}", time_start);
    let time_end = riscv::register::time::read64();
    if time_end > time_start {
        println!("<< Test-kernel: Time after operation: {:x}", time_end);
    } else {
        println!("!! Test-kernel: SBI test FAILED due to incorrect time counter");
        sbi::shutdown()
    }
}

extern "C" fn rust_trap_exception(trap_frame: &mut TrapFrame) {
    if trap_frame.tp == 0 {
        let cause = scause::read().cause();
        println!("<< Test-kernel: Value of scause: {:?}", cause);
        if cause != Trap::Exception(Exception::IllegalInstruction) {
            println!("!! Test-kernel: Wrong cause associated to illegal instruction");
            sbi::shutdown()
        }
        println!("<< Test-kernel: Illegal exception delegate success");
        sepc::write(sepc::read().wrapping_add(4));
    } else if trap_frame.tp == 4 {
        if scause::read().cause() != Trap::Interrupt(Interrupt::SupervisorSoft) {
            println!("!! Test-kernel: Wrong cause associated to S-IPI delegation");
            sbi::shutdown()
        }
    } else {
        println!("!! Test-kernel: hart {} should not trap", trap_frame.tp);
        println!("!! Test-kernel: SBI test FAILED for this hart should not trap");
        sbi::shutdown()
    }
}

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(info: &PanicInfo) -> ! {
    println!("!! Test-kernel: {}", info);
    println!("!! Test-kernel: SBI test FAILED due to panic");
    sbi::reset(sbi::RESET_TYPE_SHUTDOWN, sbi::RESET_REASON_SYSTEM_FAILURE);
    loop {}
}

const BOOT_STACK_SIZE: usize = 4096 * 4 * 8;

static mut BOOT_STACK: [u8; BOOT_STACK_SIZE] = [0; BOOT_STACK_SIZE];

#[naked]
#[link_section = ".text.entry"]
#[export_name = "_start"]
unsafe extern "C" fn entry() -> ! {
    asm!("
    # 1. set sp
    # sp = bootstack + (hartid + 1) * 0x10000
    add     t0, a0, 1
    slli    t0, t0, 14
1:  auipc   sp, %pcrel_hi({boot_stack})
    addi    sp, sp, %pcrel_lo(1b)
    add     sp, sp, t0

    # 2. jump to rust_main (absolute address)
1:  auipc   t0, %pcrel_hi({rust_main})
    addi    t0, t0, %pcrel_lo(1b)
    jr      t0
    ", 
    boot_stack = sym BOOT_STACK,
    rust_main = sym rust_main,
    options(noreturn))
}

#[cfg(target_pointer_width = "128")]
macro_rules! define_store_load {
    () => {
        ".altmacro
        .macro STORE reg, offset
            sq  \\reg, \\offset* {REGBYTES} (sp)
        .endm
        .macro LOAD reg, offset
            lq  \\reg, \\offset* {REGBYTES} (sp)
        .endm"
    };
}

#[cfg(target_pointer_width = "64")]
macro_rules! define_store_load {
    () => {
        ".altmacro
        .macro STORE reg, offset
            sd  \\reg, \\offset* {REGBYTES} (sp)
        .endm
        .macro LOAD reg, offset
            ld  \\reg, \\offset* {REGBYTES} (sp)
        .endm"
    };
}

#[cfg(target_pointer_width = "32")]
macro_rules! define_store_load {
    () => {
        ".altmacro
        .macro STORE reg, offset
            sw  \\reg, \\offset* {REGBYTES} (sp)
        .endm
        .macro LOAD reg, offset
            lw  \\reg, \\offset* {REGBYTES} (sp)
        .endm"
    };
}

#[naked]
#[link_section = ".text"]
unsafe extern "C" fn start_trap() {
    asm!(define_store_load!(), "
    .p2align 2
    addi    sp, sp, -17 * {REGBYTES}
    STORE   ra, 0
    STORE   t0, 1
    STORE   t1, 2
    STORE   t2, 3
    STORE   t3, 4
    STORE   t4, 5
    STORE   t5, 6
    STORE   t6, 7
    STORE   a0, 8
    STORE   a1, 9
    STORE   a2, 10
    STORE   a3, 11
    STORE   a4, 12
    STORE   a5, 13
    STORE   a6, 14
    STORE   a7, 15
    STORE   tp, 16
    mv      a0, sp
    call    {rust_trap_exception}
    LOAD    ra, 0
    LOAD    t0, 1
    LOAD    t1, 2
    LOAD    t2, 3
    LOAD    t3, 4
    LOAD    t4, 5
    LOAD    t5, 6
    LOAD    t6, 7
    LOAD    a0, 8
    LOAD    a1, 9
    LOAD    a2, 10
    LOAD    a3, 11
    LOAD    a4, 12
    LOAD    a5, 13
    LOAD    a6, 14
    LOAD    a7, 15
    LOAD    tp, 16
    addi    sp, sp, 17 * {REGBYTES}
    sret
    ",
    REGBYTES = const core::mem::size_of::<usize>(),
    rust_trap_exception = sym rust_trap_exception,
    options(noreturn))
}

#[repr(C)]
struct TrapFrame {
    ra: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    tp: usize,
}