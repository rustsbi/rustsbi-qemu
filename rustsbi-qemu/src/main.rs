#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_sym, asm_const)]
#![feature(generator_trait)]
#![feature(default_alloc_error_handler)]

#[macro_use]
extern crate rustsbi;
extern crate alloc;

mod clint;
mod device_tree;
mod execute;
mod ns16550a;
mod qemu_hsm;
mod test_device;

/// 特权软件信息。
struct Supervisor {
    start_addr: usize,
    opaque: usize,
}

mod constants {
    /// 特权软件入口。
    pub(crate) const SUPERVISOR_ENTRY: usize = 0x8020_0000;
    /// 每个核设置 16KiB 栈空间。
    pub(crate) const LEN_STACK_PER_HART: usize = 16 * 1024;
    /// qemu-virt 最多 8 核。
    pub(crate) const NUM_HART_MAX: usize = 8;
    /// SBI 软件全部栈空间容量。
    pub(crate) const LEN_STACK_SBI: usize = LEN_STACK_PER_HART * NUM_HART_MAX;
    /// SBI 软件堆空间容量。
    pub(crate) const LEN_HEAP_SBI: usize = LEN_STACK_SBI;
}

use constants::*;

#[cfg_attr(not(test), panic_handler)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use rustsbi::{
        reset::{RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_SHUTDOWN},
        Reset,
    };

    // 输出的信息大概是“[rustsbi-panic] hart 0 panicked at ...”
    println!("[rustsbi-panic] hart {} {info}", hart_id());
    println!("[rustsbi-panic] system shutdown scheduled due to RustSBI panic");
    test_device::get().system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
    loop {}
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
           csrw     mie,  zero
           la        sp, {stack}
           li        t0, {per_hart_stack_size}
           addi      t1,  a0, 1
        1: add       sp,  sp, t0
           addi      t1,  t1, -1
           bnez      t1,  1b
           call     {rust_main}
           call     {finalize}
        1: wfi
           j         1b
        ",
        per_hart_stack_size = const LEN_STACK_PER_HART,
        stack               =   sym SBI_STACK,
        rust_main           =   sym rust_main,
        finalize            =   sym finalize,
        options(noreturn)
    )
}

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
        unsafe { riscv::asm::wfi() };
    }
}

use spin::Once;

#[link_section = ".bss.uninit"]
static HSM: Once<qemu_hsm::QemuHsm> = Once::new();

/// rust 入口。
extern "C" fn rust_main(_hartid: usize, opaque: usize) {
    use core::sync::atomic::{AtomicBool, Ordering::AcqRel};

    unsafe { set_mtcev(early_trap as _) };

    #[link_section = ".bss.uninit"]
    static GENESIS: AtomicBool = AtomicBool::new(false);

    // 全局初始化过程
    if !GENESIS.swap(true, AcqRel) {
        // 清零 bss 段
        zero_bss();
        // 初始化堆和分配器
        init_heap();
        // 解析设备树，需要堆来保存结果里的字符串等
        let board_info = device_tree::parse(opaque);
        // 初始化外设
        clint::init(board_info.clint.start, board_info.smp);
        test_device::init(board_info.test.start);
        let uart = unsafe { ns16550a::Ns16550a::new(board_info.uart.start) };
        let hsm = HSM.call_once(|| qemu_hsm::QemuHsm::new(clint::get(), board_info.smp, opaque));
        // 初始化 SBI 服务
        rustsbi::legacy_stdio::init_legacy_stdio_embedded_hal(uart);
        rustsbi::init_ipi(clint::get());
        rustsbi::init_timer(clint::get());
        rustsbi::init_reset(test_device::get().clone());
        rustsbi::init_hsm(hsm);
        // 打印启动信息
        print!(
            "\
[rustsbi] RustSBI version {ver_sbi}, adapting to RISC-V SBI v1.0.0
{logo}
[rustsbi] Implementation     : RustSBI-QEMU Version {ver_impl}
[rustsbi] Platform Name      : {model:?}
[rustsbi] Platform SMP       : {smp}
[rustsbi] Boot HART          : {hartid}
[rustsbi] Device Tree Address: {dtb:#x}
[rustsbi] Firmware Address   : {firmware:#x}
[rustsbi] Supervisor Address : {SUPERVISOR_ENTRY:#x}
",
            ver_sbi = rustsbi::VERSION,
            logo = rustsbi::LOGO,
            ver_impl = env!("CARGO_PKG_VERSION"),
            model = board_info.model,
            smp = board_info.smp,
            hartid = hart_id(),
            dtb = opaque,
            firmware = entry as usize,
        );
    }

    let hsm = HSM.wait();
    if let Some(supervisor) = hsm.take_supervisor() {
        set_pmp();
        hsm.record_current_start_finished();
        execute::execute_supervisor(supervisor);
    }
}

/// 准备好不可恢复休眠或关闭
extern "C" fn finalize() {
    //! 在隔离的环境调用，以确保 main 中使用的堆资源完全释放
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
    #[cfg(target_pointer_width = "128")]
    type Word = u128;
    extern "C" {
        static mut sbss: Word;
        static mut ebss: Word;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
}

/// 初始化堆和分配器。
fn init_heap() {
    use buddy_system_allocator::LockedHeap;

    static mut HEAP_SPACE: [u8; LEN_HEAP_SBI] = [0; LEN_HEAP_SBI];
    #[global_allocator]
    static SBI_HEAP: LockedHeap<32> = LockedHeap::empty();

    unsafe {
        SBI_HEAP
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, HEAP_SPACE.len())
    }
}

/// 设置 PMP。
fn set_pmp() {
    use riscv::register::{pmpaddr0, pmpaddr1, pmpcfg0, Permission, Range};
    unsafe {
        pmpcfg0::set_pmp(0, Range::NAPOT, Permission::RWX, false);
        pmpcfg0::set_pmp(1, Range::NAPOT, Permission::NONE, false);
    }
    pmpaddr0::write(usize::MAX);
    pmpaddr1::write((entry as usize >> 2) | 0x10_0000 >> 2);
}

#[inline(always)]
fn hart_id() -> usize {
    riscv::register::mhartid::read()
}

#[inline(always)]
unsafe fn set_mtcev(trap_handler: usize) {
    use riscv::register::mtvec;
    mtvec::write(trap_handler, mtvec::TrapMode::Direct);
}
