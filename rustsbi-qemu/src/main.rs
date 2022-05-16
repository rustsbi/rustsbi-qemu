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
    pub(crate) const LEN_PAGE: usize = 4096; // 4KiB
    pub(crate) const PER_HART_STACK_SIZE: usize = 4 * LEN_PAGE; // 16KiB
    pub(crate) const MAX_HART_NUMBER: usize = 8; // assume 8 cores in QEMU
    pub(crate) const SBI_STACK_SIZE: usize = PER_HART_STACK_SIZE * MAX_HART_NUMBER;
    pub(crate) const SBI_HEAP_SIZE: usize = 16 * LEN_PAGE; // 64KiB
}

use constants::*;

#[cfg_attr(not(test), panic_handler)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let hart_id = riscv::register::mhartid::read();
    // 输出的信息大概是“[rustsbi-panic] hart 0 panicked at ...”
    println!("[rustsbi-panic] hart {} {}", hart_id, info);
    println!("[rustsbi-panic] system shutdown scheduled due to RustSBI panic");
    use rustsbi::Reset;
    test_device::SiFiveTest.system_reset(
        rustsbi::reset::RESET_TYPE_SHUTDOWN,
        rustsbi::reset::RESET_REASON_SYSTEM_FAILURE,
    );
    loop {}
}

lazy_static::lazy_static! {
    pub static ref HSM: qemu_hsm::QemuHsm = qemu_hsm::QemuHsm::new();
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
    static mut SBI_STACK: [u8; SBI_STACK_SIZE] = [0; SBI_STACK_SIZE];

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
        per_hart_stack_size = const PER_HART_STACK_SIZE,
        stack               =   sym SBI_STACK,
        rust_main           =   sym rust_main,
        options(noreturn)
    )
}

/// rust 入口。
extern "C" fn rust_main(hartid: usize, opaque: usize) -> ! {
    let boot_hart = race_boot_hart();
    runtime::init();
    if boot_hart {
        // 清零 bss 段
        zero_bss();
        // 初始化堆和分配器
        init_heap();
        // 解析设备树，需要堆来保存结果里的字符串等
        device_tree::init(opaque);
        // 初始化外设
        let periperals = device_tree::get();
        clint::init(periperals.clint.start);
        let uart = unsafe { ns16550a::Ns16550a::new(periperals.uart.start) };
        // 初始化 SBI 服务
        rustsbi::legacy_stdio::init_legacy_stdio_embedded_hal(uart);
        rustsbi::init_ipi(*clint::get());
        rustsbi::init_timer(*clint::get());
        rustsbi::init_reset(test_device::SiFiveTest);
        rustsbi::init_hsm(HSM.clone());
        // 打印启动信息
        println!(
            "[rustsbi] RustSBI version {}, adapting to RISC-V SBI v1.0.0",
            rustsbi::VERSION
        );
        println!("{}", rustsbi::LOGO);
        println!(
            "[rustsbi] Implementation: RustSBI-QEMU Version {}",
            env!("CARGO_PKG_VERSION")
        );
        let info = device_tree::get();
        println!("[rustsbi] Device model: {:?}", info.model);
    } else {
        qemu_hsm::pause();
    }
    delegate_interrupt_exception();
    set_pmp();
    unsafe {
        // enable wake by ipi
        riscv::register::mstatus::set_mie();
    }
    if boot_hart {
        // print hart csr configuration
        hart_csr_utils::print_hart_csrs();
        // start other harts
        let clint = crate::clint::get();
        let num_harts = device_tree::get().smp;
        for target_hart_id in 0..num_harts {
            if target_hart_id != hartid {
                clint.send_soft(target_hart_id);
            }
        }
        println!("[rustsbi] enter supervisor 0x80200000");
    }
    // start SBI environment
    execute::execute_supervisor(0x80200000, hartid, opaque, HSM.clone());
}

fn race_boot_hart() -> bool {
    use core::sync::atomic::{AtomicBool, Ordering::SeqCst};
    #[link_section = ".bss.uninit"]
    static mut BOOT_SELECTOR: AtomicBool = AtomicBool::new(false);
    unsafe { BOOT_SELECTOR.compare_exchange(false, true, SeqCst, SeqCst) }.is_ok()
}

/// 清零 bss 段。
#[inline(always)]
fn zero_bss() {
    #[cfg(target_arch = "riscv32")]
    type Word = u32;
    #[cfg(target_arch = "riscv64")]
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
    static mut HEAP_SPACE: [u8; SBI_HEAP_SIZE] = [0; SBI_HEAP_SIZE];
    #[global_allocator]
    static SBI_HEAP: LockedHeap<32> = LockedHeap::empty();

    unsafe {
        SBI_HEAP
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, SBI_HEAP_SIZE)
    }
}

// 委托中断；把S的中断全部委托给S层
fn delegate_interrupt_exception() {
    use riscv::register::{medeleg, mideleg, mie};
    unsafe {
        mideleg::set_sext();
        mideleg::set_stimer();
        mideleg::set_ssoft();
        mideleg::set_uext();
        mideleg::set_utimer();
        mideleg::set_usoft();
        medeleg::set_instruction_misaligned();
        medeleg::set_breakpoint();
        medeleg::set_user_env_call();
        medeleg::set_instruction_page_fault();
        medeleg::set_load_page_fault();
        medeleg::set_store_page_fault();
        medeleg::set_instruction_fault();
        medeleg::set_load_fault();
        medeleg::set_store_fault();
        mie::set_mext();
        // 不打开mie::set_mtimer
        mie::set_msoft();
    }
}

fn set_pmp() {
    use core::ops::Range;
    use riscv::register::{
        pmpaddr0, pmpaddr1, pmpaddr2, pmpaddr3, pmpaddr4, pmpaddr5, pmpaddr6, pmpaddr7, pmpaddr8,
        pmpcfg0,
    };

    // todo: 根据QEMU的loader device等等，设置这里的权限配置
    let periperals = device_tree::get();
    let memory = &periperals.memory;
    let rtc = &periperals.rtc;
    let uart = &periperals.uart;
    let test = &periperals.test;
    let pci = &periperals.pci;
    let clint = &periperals.clint;
    let plic = &periperals.plic;

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
