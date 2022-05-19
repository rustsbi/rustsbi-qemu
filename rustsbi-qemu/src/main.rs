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
mod hart_csr_utils;
mod ns16550a;
mod qemu_hsm;
mod test_device;

mod constants {
    /// 特权软件入口
    pub(crate) const SUPERVISOR_ENTRY: usize = 0x8020_0000;
    /// 每个核设置 16KiB 栈空间
    pub(crate) const LEN_STACK_PER_HART: usize = 16 * 1024;
    /// qemu-virt 最多 8 核
    pub(crate) const NUM_HART_MAX: usize = 8;
    /// SBI 软件全部栈空间容量
    pub(crate) const LEN_STACK_SBI: usize = LEN_STACK_PER_HART * NUM_HART_MAX;
    /// SBI 软件堆空间容量
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

#[link_section = ".text.exception"]
extern "C" fn exception() -> ! {
    println!(
        "{:?} {} {} {:#x?}",
        riscv::register::mcause::read().cause(),
        riscv::register::mepc::read(),
        riscv::register::mtval::read(),
        riscv::register::mstatus::read(),
    );
    loop {
        unsafe { riscv::asm::nop() };
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
           mv    tp,  a0
           la    sp, {stack}
           li    t0, {per_hart_stack_size}
           addi  t1,  a0, 1
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

use spin::Once;

#[link_section = ".bss.uninit"]
static BOARD_INFO: Once<device_tree::BoardInfo> = Once::new();

#[link_section = ".bss.uninit"]
static HSM: Once<qemu_hsm::QemuHsm> = Once::new();

#[link_section = ".bss.uninit"]
static GENESIS: Once<()> = Once::new();

/// rust 入口。
extern "C" fn rust_main(_hartid: usize, opaque: usize) {
    use riscv::register::mtvec;
    unsafe { mtvec::write(exception as _, mtvec::TrapMode::Direct) };
    // 全局初始化过程
    let genesis = genesis();
    if genesis {
        // 清零 bss 段
        zero_bss();
        // 初始化堆和分配器
        init_heap();
        // 解析设备树，需要堆来保存结果里的字符串等
        let board_info = BOARD_INFO.call_once(|| device_tree::parse(opaque));
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
        println!(
            "\
[rustsbi] RustSBI version {ver_sbi}, adapting to RISC-V SBI v1.0.0
{logo}
[rustsbi] Implementation: RustSBI-QEMU Version {ver_impl}
[rustsbi] Device model: {model:?}",
            ver_sbi = rustsbi::VERSION,
            logo = rustsbi::LOGO,
            ver_impl = env!("CARGO_PKG_VERSION"),
            model = board_info.model
        );
        GENESIS.call_once(|| ());
    }

    GENESIS.wait();
    set_pmp(BOARD_INFO.wait());
    if genesis {
        hart_csr_utils::print_hart_csrs();
        println!("[rustsbi] enter supervisor {SUPERVISOR_ENTRY:#x}");
    }

    execute::execute_supervisor(HSM.wait());
}

extern "C" fn finalize() {
    use riscv::{
        interrupt,
        register::{mie, mip, mtvec},
    };
    unsafe {
        mtvec::write(entry as _, mtvec::TrapMode::Direct);
        mip::clear_msoft();
        mie::set_msoft();
    }
    HSM.wait().record_ready_to_reboot();
    unsafe { interrupt::enable() };
}

/// 抢夺启动权。
fn genesis() -> bool {
    use core::sync::atomic::{AtomicBool, Ordering::SeqCst};
    #[link_section = ".bss.uninit"]
    static BOOT_SELECTOR: AtomicBool = AtomicBool::new(false);
    BOOT_SELECTOR
        .compare_exchange(false, true, SeqCst, SeqCst)
        .is_ok()
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

    #[link_section = ".bss.uninit"]
    static mut HEAP_SPACE: [u8; LEN_HEAP_SBI] = [0; LEN_HEAP_SBI];
    #[link_section = ".bss.uninit"]
    #[global_allocator]
    static SBI_HEAP: LockedHeap<32> = LockedHeap::empty();

    unsafe {
        SBI_HEAP
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, HEAP_SPACE.len())
    }
}

/// 设置 PMP。
///
/// FIXME 最好能实现一个排序+合并连续区域的复杂算法，尽量将地址段配置为 NAPOT 以节省 PMP 段，不过全部 TOR 也够用了
fn set_pmp(board_info: &device_tree::BoardInfo) {
    use riscv::register::{
        pmpaddr0, pmpaddr1, pmpaddr10, pmpaddr11, pmpaddr12, pmpaddr13, pmpaddr14, pmpaddr15,
        pmpaddr2, pmpaddr3, pmpaddr4, pmpaddr5, pmpaddr6, pmpaddr7, pmpaddr8, pmpaddr9, pmpcfg0,
        pmpcfg2,
    };

    let memory = &board_info.memory;
    let rtc = &board_info.rtc;
    let uart = &board_info.uart;
    let test = &board_info.test;
    let pci = &board_info.pci;
    let clint = &board_info.clint;
    let plic = &board_info.plic;

    let mut pmpcfg0 = PmpCfg::ZERO;
    // rtc
    pmpcfg0.set_next(0);
    pmpaddr0::write(rtc.start >> 2);
    pmpcfg0.set_next(0b1011);
    pmpaddr1::write(rtc.end >> 2);
    // uart
    pmpcfg0.set_next(0);
    pmpaddr2::write(uart.start >> 2);
    pmpcfg0.set_next(0b1011);
    pmpaddr3::write(uart.end >> 2);
    // test
    pmpcfg0.set_next(0);
    pmpaddr4::write(test.start >> 2);
    pmpcfg0.set_next(0b1011);
    pmpaddr5::write(test.end >> 2);
    // pci
    pmpcfg0.set_next(0);
    pmpaddr6::write(pci.start >> 2);
    pmpcfg0.set_next(0b1011);
    pmpaddr7::write(pci.end >> 2);
    // cfg
    pmpcfg0::write(pmpcfg0.bits());

    let mut pmpcfg2 = PmpCfg::ZERO;
    // clint
    pmpcfg2.set_next(0);
    pmpaddr8::write(clint.start >> 2);
    pmpcfg2.set_next(0b1011);
    pmpaddr9::write(clint.end >> 2);
    // plic
    pmpcfg2.set_next(0);
    pmpaddr10::write(plic.start >> 2);
    pmpcfg2.set_next(0b1011);
    pmpaddr11::write(plic.end >> 2);
    // virtio_mmio
    pmpcfg2.set_next(0);
    pmpaddr12::write(0x1000_1000 >> 2);
    pmpcfg2.set_next(0b1011);
    pmpaddr13::write(0x1000_9000 >> 2);
    // memory
    pmpcfg2.set_next(0);
    pmpaddr14::write(SUPERVISOR_ENTRY >> 2);
    pmpcfg2.set_next(0b1111);
    pmpaddr15::write(memory.end >> 2);
    // cfg
    pmpcfg2::write(pmpcfg2.bits());
}

struct PmpCfg(usize, usize);

impl PmpCfg {
    const ZERO: Self = Self(0, 0);

    fn set_next(&mut self, value: u8) {
        self.0 |= (value as usize) << self.1;
        self.1 += 8;
    }

    fn bits(&self) -> usize {
        self.0
    }
}

#[inline(always)]
fn hart_id() -> usize {
    let ans: usize;
    unsafe { core::arch::asm!("mv {}, tp", out(reg) ans) }
    ans
}
