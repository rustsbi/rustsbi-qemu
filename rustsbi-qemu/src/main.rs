#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm)]
#![feature(generator_trait)]
#![feature(default_alloc_error_handler)]

extern crate alloc;

mod clint;
mod ns16550a;
mod test_device;
mod execute;
mod runtime;
mod count_harts;
mod feature;
mod hart_csr_utils;

use core::panic::PanicInfo;
use buddy_system_allocator::LockedHeap;

use rustsbi::println;

const SBI_HEAP_SIZE: usize = 64 * 1024;
static mut HEAP_SPACE: [u8; SBI_HEAP_SIZE] = [0; SBI_HEAP_SIZE];
#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(info: &PanicInfo) -> ! {
    let hart_id = riscv::register::mhartid::read();
    // 输出的信息大概是“[rustsbi-panic] hart 0 panicked at ...”
    println!("[rustsbi-panic] hart {} {}", hart_id, info);
    println!("[rustsbi-panic] system shutdown scheduled due to RustSBI panic");
    use rustsbi::Reset;
    test_device::Reset.system_reset(
        rustsbi::reset::RESET_TYPE_SHUTDOWN,
        rustsbi::reset::RESET_REASON_SYSTEM_FAILURE
    );
    loop { }
}

extern "C" fn rust_main(hartid: usize, dtb_pa: usize) -> ! {
    runtime::init();
    if hartid == 0 {
        init_heap();
        init_legacy_stdio();
        init_clint();
        init_test_device();
        println!("[rustsbi] RustSBI version {}", rustsbi::VERSION);
        println!("{}", rustsbi::LOGO);
        println!("[rustsbi] Implementation: RustSBI-QEMU Version {}", env!("CARGO_PKG_VERSION"));
        unsafe { count_harts::init_hart_count(dtb_pa) };
    }
    delegate_interrupt_exception();
    if hartid == 0 {
        hart_csr_utils::print_misa_medeleg_mideleg();
        println!("[rustsbi] enter supervisor 0x80200000");
    }
    execute::execute_supervisor(0x80200000, hartid, dtb_pa);
}

fn init_heap() {
    unsafe {
        HEAP.lock().init(
            HEAP_SPACE.as_ptr() as usize, SBI_HEAP_SIZE
        )
    }
}

fn init_legacy_stdio() {
    let serial = ns16550a::Ns16550a::new(0x10000000, 0, 11_059_200, 115200);
    use rustsbi::legacy_stdio::init_legacy_stdio_embedded_hal;
    init_legacy_stdio_embedded_hal(serial);
}

fn init_clint() {
    let clint = clint::Clint::new(0x2000000 as *mut u8);
    use rustsbi::init_ipi;
    init_ipi(clint);
    let clint = clint::Clint::new(0x2000000 as *mut u8);
    use rustsbi::init_timer;
    init_timer(clint);
}

fn init_test_device() {
    use rustsbi::init_reset;
    init_reset(test_device::Reset);
}

// 委托终端；把S的中断全部委托给S层
fn delegate_interrupt_exception() {
    use riscv::register::{mideleg, medeleg, mie};
    unsafe {
        mideleg::set_sext();
        mideleg::set_stimer();
        mideleg::set_ssoft();
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


const HART_STACK_SIZE: usize = 4 * 4096; // 16KiB
const STACK_SIZE: usize = 8 * HART_STACK_SIZE; // assume 8 cores in QEMU

#[link_section = ".bss.stack"]
static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

#[naked]
#[link_section = ".text.entry"] 
#[export_name = "_start"]
unsafe extern "C" fn entry() -> ! {
    asm!(
    // 1. set sp
    // sp = bootstack + (hartid + 1) * HART_STACK_SIZE
    "
    la      sp, {stack}
    li      t0, {per_hart_stack_size}
    addi    t1, a0, 1
1:  add     sp, sp, t0
    addi    t1, t1, -1
    bnez    t1, 1b
    ",
    // 2. jump to rust_main (absolute address)
    "j      {rust_main}", 
    per_hart_stack_size = const HART_STACK_SIZE,
    stack = sym STACK, 
    rust_main = sym rust_main,
    options(noreturn))
}
