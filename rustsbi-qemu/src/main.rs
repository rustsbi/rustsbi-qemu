#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_sym, asm_const)]
#![feature(generator_trait)]
#![feature(default_alloc_error_handler)]

extern crate alloc;

#[macro_use]
extern crate rustsbi;

mod clint;
mod device_tree;
mod execute;
mod feature;
mod hart_csr_utils;
mod ns16550a;
mod prv_mem;
mod qemu_hsm;
mod runtime;
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
    use riscv::register::mhartid;
    use rustsbi::{
        reset::{RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_SHUTDOWN},
        Reset,
    };

    // 输出的信息大概是“[rustsbi-panic] hart 0 panicked at ...”
    println!("[rustsbi-panic] hart {} {info}", mhartid::read());
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
           csrw  mie,  zero
           la     sp, {stack}
           li     t0, {per_hart_stack_size}
           addi   t1,  a0, 1
        1: add    sp,  sp, t0
           addi   t1,  t1, -1
           bnez   t1,  1b
           j    {rust_main}
        ",
        per_hart_stack_size = const LEN_STACK_PER_HART,
        stack               =   sym SBI_STACK,
        rust_main           =   sym rust_main,
        options(noreturn)
    )
}

/// rust 入口。
extern "C" fn rust_main(hartid: usize, opaque: usize) -> ! {
    use spin::Once;

    static BOARD_INFO: Once<device_tree::BoardInfo> = Once::new();
    static HSM: Once<qemu_hsm::QemuHsm> = Once::new();

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
        let hsm = HSM.call_once(|| qemu_hsm::QemuHsm::new(clint::get()));
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
    }

    runtime::init();
    HSM.wait().record_current_start_finished();
    set_pmp(BOARD_INFO.wait());
    delegate_supervisor_trap();
    enable_mint();

    if genesis {
        hart_csr_utils::print_hart_csrs();
        println!("[rustsbi] enter supervisor {SUPERVISOR_ENTRY}");
        execute::execute_supervisor(SUPERVISOR_ENTRY, hartid, opaque, HSM.wait());
    } else {
        use rustsbi::Hsm;
        HSM.wait().hart_stop();
        unreachable!()
    }
}

/// 抢夺启动权。
fn genesis() -> bool {
    use core::sync::atomic::{AtomicBool, Ordering::SeqCst};
    #[link_section = ".bss.uninit"]
    static mut BOOT_SELECTOR: AtomicBool = AtomicBool::new(false);
    unsafe { BOOT_SELECTOR.compare_exchange(false, true, SeqCst, SeqCst) }.is_ok()
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
    static mut HEAP_SPACE: [u8; LEN_HEAP_SBI] = [0; LEN_HEAP_SBI];
    #[global_allocator]
    static SBI_HEAP: LockedHeap<32> = LockedHeap::empty();

    unsafe {
        SBI_HEAP
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, HEAP_SPACE.len())
    }
}

/// 委托中断
fn delegate_supervisor_trap() {
    use core::arch::asm;
    use riscv::register::medeleg;
    unsafe {
        asm!("csrrw zero, mideleg, {}", in(reg) usize::MAX);
        asm!("csrrw zero, medeleg, {}", in(reg) usize::MAX);
        medeleg::clear_illegal_instruction();
        medeleg::clear_load_misaligned();
        medeleg::clear_store_misaligned();
        medeleg::clear_supervisor_env_call();
        medeleg::clear_machine_env_call();
    }
}

/// 使能中断。
fn enable_mint() {
    use riscv::{interrupt, register::mie};
    unsafe {
        // mie::set_mtimer();
        mie::set_mext();
        mie::set_msoft();
        interrupt::enable();
    }
}

/// 设置 PMP。
///
/// FIXME 需要判断一个外设区域是否能用 NAPOT 表示，最好能实现一个排序+合并连续区域的复杂算法
fn set_pmp(board_info: &device_tree::BoardInfo) {
    use core::ops::Range;
    use riscv::register::{
        pmpaddr0, pmpaddr1, pmpaddr2, pmpaddr3, pmpaddr4, pmpaddr5, pmpaddr6, pmpaddr7, pmpaddr8,
        pmpcfg0,
    };

    // todo: 根据QEMU的loader device等等，设置这里的权限配置
    let memory = &board_info.memory;
    let rtc = &board_info.rtc;
    let uart = &board_info.uart;
    let test = &board_info.test;
    let pci = &board_info.pci;
    let clint = &board_info.clint;
    let plic = &board_info.plic;

    fn calc_pmpaddr_napot(range: &Range<usize>) -> usize {
        let start = range.start;
        let len = range.len();
        let len = if len.count_ones() == 1 {
            len
        } else {
            let mut i = 1;
            while i < range.len() {
                i <<= 1;
            }
            i
        };
        (start >> 2) | ((len >> 2) - 1)
    }

    let mut pmpcfg0 = PmpCfg::ZERO;
    // memory
    pmpcfg0.set_next(0b11111);
    pmpaddr0::write(calc_pmpaddr_napot(memory));
    // rtc
    pmpcfg0.set_next(0b11011);
    pmpaddr1::write(calc_pmpaddr_napot(rtc));
    // uart
    pmpcfg0.set_next(0b11011);
    pmpaddr2::write(calc_pmpaddr_napot(uart));
    // test
    pmpcfg0.set_next(0b11011);
    pmpaddr3::write(calc_pmpaddr_napot(test));
    // pci
    pmpcfg0.set_next(0b11011);
    pmpaddr4::write(calc_pmpaddr_napot(pci));
    // clint
    pmpcfg0.set_next(0b11011);
    pmpaddr5::write(calc_pmpaddr_napot(clint));
    // plic
    pmpcfg0.set_next(0b11011);
    pmpaddr6::write(calc_pmpaddr_napot(plic));
    // virtio_mmio
    pmpcfg0.set_next(0b01011);
    pmpaddr7::write(0x1000_1000 >> 2);
    pmpcfg0.set_next(0b00000);
    pmpaddr8::write(0x1000_9000 >> 2);
    // cfg
    pmpcfg0::write(pmpcfg0.bits());
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
    riscv::register::mhartid::read()
}
