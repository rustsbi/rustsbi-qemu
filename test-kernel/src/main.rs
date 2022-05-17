//! A test kernel to test RustSBI function on all platforms

#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc,
    stvec::{self, TrapMode},
};

extern crate sbi_rt as sbi;

#[macro_use]
mod console;
mod test;

mod constants {
    pub(crate) const LEN_PAGE: usize = 4096; // 4KiB
    pub(crate) const PER_HART_STACK_SIZE: usize = 4 * LEN_PAGE; // 16KiB
    pub(crate) const MAX_HART_NUMBER: usize = 8; // assume 8 cores in QEMU
    pub(crate) const STACK_SIZE: usize = PER_HART_STACK_SIZE * MAX_HART_NUMBER;
    pub(crate) const HEAP_SIZE: usize = 16 * LEN_PAGE; // 64KiB
}

use constants::*;

#[cfg_attr(not(test), panic_handler)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use sbi::{system_reset, RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_SHUTDOWN};

    let hard_id: usize;
    unsafe { asm!("mv {}, tp", out(reg) hard_id) };
    println!("[test-kernel-panic] hart {hard_id} {info}");
    println!("[test-kernel-panic] SBI test FAILED due to panic");
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
    loop {}
}

/// 内核入口。
///
/// # Safety
///
/// 裸函数。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start(hartid: usize, device_tree_paddr: usize) -> ! {
    asm!(
        "csrw sie, zero",      // 关中断
        "call {select_stack}", // 设置启动栈
        "j    {main}",         // 进入 rust
        select_stack = sym select_stack,
        main = sym primary_rust_main,
        options(noreturn)
    )
}

/// 副核入口。此前副核被 SBI 阻塞。
///
/// # Safety
///
/// 裸函数。
#[naked]
unsafe extern "C" fn secondary_hart_start(hartid: usize) -> ! {
    asm!(
        "csrw sie, zero",      // 关中断
        "call {select_stack}", // 设置启动栈
        "j    {main}",         // 进入 rust
        select_stack = sym select_stack,
        main = sym secondary_rust_main,
        options(noreturn)
    )
}

extern "C" fn primary_rust_main(hartid: usize, dtb_pa: usize) -> ! {
    zero_bss();

    println!(
        r"
 _____         _     _  __                    _
|_   _|__  ___| |_  | |/ /___ _ __ _ __   ___| |
  | |/ _ \/ __| __| | ' // _ \ '__| '_ \ / _ \ |
  | |  __/\__ \ |_  | . \  __/ |  | | | |  __/ |
  |_|\___||___/\__| |_|\_\___|_|  |_| |_|\___|_|
================================================
boot hart id = {hartid}, dtb physical address = {dtb_pa:#x}"
    );
    test::base_extension();
    test::sbi_ins_emulation();

    unsafe { stvec::write(start_trap as usize, TrapMode::Direct) };
    // init_heap();

    println!(">> Test-kernel: Trigger illegal exception");
    unsafe { asm!("csrw mcycle, x0") }; // mcycle cannot be written, this is always a 4-byte illegal instruction

    // if hartid == 0 {
    //     let sbi_ret = sbi::hart_stop();
    //     println!(">> Stop hart 3, return value {:?}", sbi_ret);
    //     for i in 0..5 {
    //         let sbi_ret = sbi::hart_get_status(i);
    //         println!(">> Hart {} state return value: {:?}", i, sbi_ret);
    //     }
    // } else if hartid == 1 {
    //     let sbi_ret = sbi::hart_suspend(0x00000000, 0, 0);
    //     println!(
    //         ">> Start test for hart {}, retentive suspend return value {:?}",
    //         hartid, sbi_ret
    //     );
    // } else if hartid == 2 {
    //     /* resume_addr should be physical address, and here pa == va */
    //     let sbi_ret = sbi::hart_suspend(0x80000000, hart_2_resume as usize, 0x4567890a);
    //     println!(">> Error for non-retentive suspend: {:?}", sbi_ret);
    //     loop {}
    // // } else if hartid == 4 {
    // // unsafe { stvec::write(start_trap_addr, TrapMode::Direct) };
    // // unsafe { sstatus::set_sie() };
    // // unsafe { sie::set_ssoft() };
    // // loop {} // wait for S-IPI
    // // println!(">> Test-kernel: SBI S-IPI delegation success");
    // // println!("<< Test-kernel: All hart SBI test SUCCESS, shutdown");
    // // todo: S-IPI
    // } else {
    //     // hartid == 3
    //     loop {}
    // }
    // if hartid == 0 {
    //     println!(
    //         "<< Test-kernel: test for hart {} success, wake another hart",
    //         hartid
    //     );
    //     let sbi_ret = sbi::send_ipi(0b10, 0); // wake hart 1
    //     println!(">> Wake hart 1, sbi return value {:?}", sbi_ret);
    //     loop {} // wait for machine shutdown
    // } else if hartid == 1 {
    //     // send software IPI to activate hart 2
    //     let sbi_ret = sbi::send_ipi(0b1, 2);
    //     println!(">> Wake hart 2, sbi return value {:?}", sbi_ret);
    //     loop {}
    // } else {
    //     // hartid == 2 || hartid == 3 || hartid == 4
    //     unreachable!()
    // }
    sbi::legacy::shutdown();
    unreachable!()
}

extern "C" fn secondary_rust_main(hart_id: usize) -> ! {
    sbi::hart_stop();
    unreachable!()
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
    sbi::legacy::shutdown()
    // todo: S-IPI
    // println!(">> Send IPI to hart 4, should delegate IPI to S-level");
    // let _ = sbi::send_ipi(0b1, 4); // IPI to hart 4
    // loop {} // wait for machine shutdown
}

extern "C" fn rust_trap_exception(trap_frame: &mut TrapFrame) {
    if trap_frame.tp == 0 {
        let cause = scause::read().cause();
        println!("<< Test-kernel: Value of scause: {:?}", cause);
        if cause != Trap::Exception(Exception::IllegalInstruction) {
            println!("!! Test-kernel: Wrong cause associated to illegal instruction");
            sbi::legacy::shutdown()
        }
        println!("<< Test-kernel: Illegal exception delegate success");
        sepc::write(sepc::read().wrapping_add(4));
    } else if trap_frame.tp == 4 {
        if scause::read().cause() != Trap::Interrupt(Interrupt::SupervisorSoft) {
            println!("!! Test-kernel: Wrong cause associated to S-IPI delegation");
            sbi::legacy::shutdown()
        }
    } else {
        println!("!! Test-kernel: hart {} should not trap", trap_frame.tp);
        println!("!! Test-kernel: SBI test FAILED for this hart should not trap");
        sbi::legacy::shutdown()
    }
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
#[link_section = ".text.trap_handler"]
unsafe extern "C" fn start_trap() {
    asm!(define_store_load!(), "
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

/// 根据硬件线程号设置启动栈。
///
/// # Safety
///
/// 裸函数。
#[naked]
unsafe extern "C" fn select_stack(hartid: usize) {
    #[link_section = ".bss.uninit"]
    static mut BOOT_STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    asm!("
           mv   tp, a0
           addi t0, a0,  1
           la   sp, {stack}
           li   t1, {len_per_hart}
        1: add  sp, sp, t1
           addi t0, t0, -1
           bnez t0, 1b
           ret
        ",
        stack = sym BOOT_STACK,
        len_per_hart = const PER_HART_STACK_SIZE,
        options(noreturn)
    )
}

/// 清零 bss 段。
#[inline(always)]
fn zero_bss() {
    #[cfg(target_pointer_width = "32")]
    type Word = u32;
    #[cfg(target_pointer_width = "64")]
    type Word = u64;
    extern "C" {
        static mut sbss: Word;
        static mut ebss: Word;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
}

/// 初始化堆和分配器。
fn init_heap() {
    use buddy_system_allocator::LockedHeap;

    #[link_section = ".bss.uninit"]
    static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    #[global_allocator]
    static SBI_HEAP: LockedHeap<32> = LockedHeap::empty();

    unsafe {
        SBI_HEAP
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, HEAP_SPACE.len())
    }
}
