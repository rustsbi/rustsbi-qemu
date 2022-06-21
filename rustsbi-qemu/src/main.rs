#![feature(naked_functions, asm_sym, asm_const)]
#![no_std]
#![no_main]
#![deny(warnings)]

#[macro_use] // for print
extern crate rustsbi;

mod clint;
mod device_tree;
mod execute;
mod hart_csr_utils;
mod ns16550a;
mod qemu_hsm;
mod qemu_test;

mod constants {
    /// 特权软件入口。
    pub(crate) const SUPERVISOR_ENTRY: usize = 0x8020_0000;
    /// 每个核设置 16KiB 栈空间。
    pub(crate) const LEN_STACK_PER_HART: usize = 16 * 1024;
    /// qemu-virt 最多 8 核。
    pub(crate) const NUM_HART_MAX: usize = 8;
    /// SBI 软件全部栈空间容量。
    pub(crate) const LEN_STACK_SBI: usize = LEN_STACK_PER_HART * NUM_HART_MAX;
}

use constants::*;

/// 特权软件信息。
struct Supervisor {
    start_addr: usize,
    opaque: usize,
}

#[cfg_attr(not(test), panic_handler)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use rustsbi::{
        spec::srst::{RESET_REASON_NO_REASON, RESET_TYPE_SHUTDOWN},
        Reset,
    };
    // 输出的信息大概是“[rustsbi-panic] hart 0 panicked at ...”
    println!("[rustsbi-panic] hart {} {info}", hart_id());
    println!("[rustsbi-panic] system shutdown scheduled due to RustSBI panic");
    qemu_test::get().system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
    loop {
        core::hint::spin_loop();
    }
}

/// 入口。
///
/// 1. 关中断
/// 2. 设置启动栈
/// 3. 跳转到 rust 入口函数
///
/// # Safety
///
/// 裸函数。
#[naked]
#[link_section = ".text.entry"]
#[export_name = "_start"]
unsafe extern "C" fn entry(hartid: usize, opaque: usize) -> ! {
    #[link_section = ".bss.uninit"]
    static mut SBI_STACK: [u8; LEN_STACK_SBI] = [0; LEN_STACK_SBI];

    core::arch::asm!("
           csrw mie,  zero
           csrr  a0,  mhartid
           la    sp, {stack}
           li    t0, {per_hart_stack_size}
           addi  t1,  a0,  1
        1: add   sp,  sp, t0
           addi  t1,  t1, -1
           bnez  t1,  1b
           call {rust_main}
           call {finalize}
        1: wfi
           j     1b
        ",
        per_hart_stack_size = const LEN_STACK_PER_HART,
        stack               =   sym SBI_STACK,
        rust_main           =   sym rust_main,
        finalize            =   sym finalize,
        options(noreturn)
    )
}

/// 真正的异常处理函数设置好之前，使用这个处理异常。
#[link_section = ".text.early_trap"]
extern "C" fn early_trap() -> ! {
    print!(
        "\
{:?} at hart[{}]{:#x}
{:#x?}
",
        riscv::register::mcause::read().cause(),
        hart_id(),
        riscv::register::mepc::read(),
        riscv::register::mstatus::read(),
    );
    loop {
        core::hint::spin_loop();
    }
}

use core::sync::atomic::{AtomicBool, Ordering::AcqRel};
use device_tree::BoardInfo;
use spin::Once;

static HSM: Once<qemu_hsm::QemuHsm> = Once::new();

/// rust 入口。
extern "C" fn rust_main(_hartid: usize, opaque: usize) {
    unsafe { set_mtvec(early_trap as _) };

    #[link_section = ".bss.uninit"] // 以免清零
    static GENESIS: AtomicBool = AtomicBool::new(false);

    static SERIAL: Once<ns16550a::Ns16550a> = Once::new();
    static BOARD_INFO: Once<BoardInfo> = Once::new();
    static CSR_PRINT: AtomicBool = AtomicBool::new(false);

    // 全局初始化过程
    if !GENESIS.swap(true, AcqRel) {
        // 清零 bss 段
        zero_bss();
        // 解析设备树
        let board_info = BOARD_INFO.call_once(|| device_tree::parse(opaque));
        // 初始化外设
        rustsbi::legacy_stdio::init_legacy_stdio(
            SERIAL.call_once(|| unsafe { ns16550a::Ns16550a::new(board_info.uart.start) }),
        );

        clint::init(board_info.clint.start);
        qemu_test::init(board_info.test.start);
        let hsm = HSM.call_once(|| qemu_hsm::QemuHsm::new(clint::get(), NUM_HART_MAX, opaque));
        // 初始化 SBI 服务
        rustsbi::init_ipi(clint::get());
        rustsbi::init_timer(clint::get());
        rustsbi::init_reset(qemu_test::get());
        rustsbi::init_hsm(hsm);
        // 打印启动信息
        print!(
            "\
[rustsbi] RustSBI version {ver_sbi}, adapting to RISC-V SBI v1.0.0
{logo}
[rustsbi] Implementation     : RustSBI-QEMU Version {ver_impl}
[rustsbi] Platform Name      : {model}
[rustsbi] Platform SMP       : {smp}
[rustsbi] Platform Memory    : {mem:#x?}
[rustsbi] Boot HART          : {hartid}
[rustsbi] Device Tree Region : {dtb:#x?}
[rustsbi] Firmware Address   : {firmware:#x}
[rustsbi] Supervisor Address : {SUPERVISOR_ENTRY:#x}
",
            ver_sbi = rustsbi::VERSION,
            logo = rustsbi::logo(),
            ver_impl = env!("CARGO_PKG_VERSION"),
            model = board_info.model,
            smp = board_info.smp,
            mem = board_info.mem,
            hartid = hart_id(),
            dtb = board_info.dtb,
            firmware = entry as usize,
        );
    }

    let hsm = HSM.wait();
    if let Some(supervisor) = hsm.take_supervisor() {
        // 设置并打印 pmp
        set_pmp(BOARD_INFO.wait());
        if !CSR_PRINT.swap(true, AcqRel) {
            hart_csr_utils::print_pmps();
        }
        execute::execute_supervisor(hsm, supervisor);
    }
}

/// 准备好不可恢复休眠或关闭
///
/// 在隔离的环境（汇编）调用，以确保 main 中使用的堆资源完全释放。
/// （只是作为示例，因为这个版本完全不使用堆）
extern "C" fn finalize() {
    HSM.wait().finallize_before_stop();
    unsafe { riscv::interrupt::enable() };
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

/// 设置 PMP。
fn set_pmp(board_info: &BoardInfo) {
    use riscv::register::{pmpaddr0, pmpaddr1, pmpaddr2, pmpaddr3, pmpcfg0, Permission, Range};
    let mem = &board_info.mem;
    unsafe {
        pmpcfg0::set_pmp(0, Range::OFF, Permission::NONE, false);
        pmpaddr0::write(0);
        // 外设
        pmpcfg0::set_pmp(1, Range::TOR, Permission::RW, false);
        pmpaddr1::write(mem.start >> 2);
        // SBI
        pmpcfg0::set_pmp(2, Range::TOR, Permission::NONE, false);
        pmpaddr2::write(SUPERVISOR_ENTRY >> 2);
        //主存
        pmpcfg0::set_pmp(3, Range::TOR, Permission::RWX, false);
        pmpaddr3::write(mem.end >> 2);
    }
}

#[inline(always)]
fn hart_id() -> usize {
    riscv::register::mhartid::read()
}

#[inline(always)]
unsafe fn set_mtvec(trap_handler: usize) {
    use riscv::register::mtvec;
    mtvec::write(trap_handler, mtvec::TrapMode::Direct);
}
