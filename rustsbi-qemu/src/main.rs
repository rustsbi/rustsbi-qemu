#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_sym, asm_const)]
#![feature(generator_trait)]
#![feature(default_alloc_error_handler)]

extern crate alloc;
#[macro_use]
extern crate rustsbi;

use core::arch::asm;
use core::panic::PanicInfo;

use buddy_system_allocator::LockedHeap;

mod clint;
mod count_harts;
mod execute;
mod feature;
mod hart_csr_utils;
mod ns16550a;
mod prv_mem;
mod qemu_hsm;
mod qemu_pmu;
mod runtime;
mod test_device;
mod trap;

const PER_HART_STACK_SIZE: usize = 4 * 4096; // 16KiB
const SBI_STACK_SIZE: usize = 8 * PER_HART_STACK_SIZE; // assume 8 cores in QEMU
#[link_section = ".bss.uninit"]
static mut SBI_STACK: [u8; SBI_STACK_SIZE] = [0; SBI_STACK_SIZE];

const SBI_HEAP_SIZE: usize = 64 * 1024; // 64KiB
#[link_section = ".bss.uninit"]
static mut HEAP_SPACE: [u8; SBI_HEAP_SIZE] = [0; SBI_HEAP_SIZE];
#[global_allocator]
static SBI_HEAP: LockedHeap<32> = LockedHeap::empty();

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(info: &PanicInfo) -> ! {
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

extern "C" fn rust_main(hartid: usize, opqaue: usize) -> ! {
    runtime::init();
    if hartid == 0 {
        init_heap();
        init_legacy_stdio();
        init_clint();
        init_test_device();
        println!(
            "[rustsbi] RustSBI version {}, adapting to RISC-V SBI v0.3",
            rustsbi::VERSION
        );
        println!("{}", rustsbi::LOGO);
        println!(
            "[rustsbi] Implementation: RustSBI-QEMU Version {}",
            env!("CARGO_PKG_VERSION")
        );
        unsafe { count_harts::init_hart_count(opqaue) };
        // initialize hsm module
        rustsbi::init_hsm(HSM.clone());
    } else {
        qemu_hsm::pause();
    }
    delegate_interrupt_exception();
    set_pmp();
    unsafe {
        // enable wake by ipi
        riscv::register::mstatus::set_mie();
    }
    if hartid == 0 {
        // print hart csr configuration
        hart_csr_utils::print_hart_csrs();
        // start other harts
        let clint = clint::Clint::new(0x2000000 as *mut u8);
        let num_harts = *{ count_harts::NUM_HARTS.lock() };
        for target_hart_id in 0..num_harts {
            if target_hart_id != 0 {
                clint.send_soft(target_hart_id);
            }
        }
        println!("[rustsbi] enter supervisor 0x80200000");
    }
    // start SBI environment
    execute::execute_supervisor(0x80200000, hartid, opqaue, HSM.clone());
}

fn init_heap() {
    unsafe {
        SBI_HEAP
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, SBI_HEAP_SIZE)
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
    init_reset(test_device::SiFiveTest);
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
    // todo: 根据QEMU的loader device等等，设置这里的权限配置
    // read fdt tree value, parse, and calculate proper pmp configuration for this device tree (issue #7)
    // integrate with `count_harts`
    //
    // Qemu MMIO config ref: https://github.com/qemu/qemu/blob/master/hw/riscv/virt.c#L46
    //
    // About PMP:
    //
    // CSR: pmpcfg0(0x3A0)~pmpcfg15(0x3AF); pmpaddr0(0x3B0)~pmpaddr63(0x3EF)
    // pmpcfg packs pmp entries each of which is of 8-bit
    // on RV64 only even pmpcfg CSRs(0,2,...,14) are available, each of which contains 8 PMP
    // entries
    // every pmp entry and its corresponding pmpaddr describe a pmp region
    //
    // layout of PMP entries:
    // ------------------------------------------------------
    //  7   |   [5:6]   |   [3:4]   |   2   |   1   |   0   |
    //  L   |   0(WARL) |   A       |   X   |   W   |   R   |
    // ------------------------------------------------------
    // A = OFF(0), disabled;
    // A = TOR(top of range, 1), match address y so that pmpaddr_{i-1}<=y<pmpaddr_i irrespective of
    // the value pmp entry i-1
    // A = NA4(naturally aligned 4-byte region, 2), only support a 4-byte pmp region
    // A = NAPOT(naturally aligned power-of-two region, 3), support a >=8-byte pmp region
    // When using NAPOT to match a address range [S,S+L), then the pmpaddr_i should be set to (S>>2)|((L>>2)-1)
    let calc_pmpaddr = |start_addr: usize, length: usize| {
        (start_addr >> 2) | ((length >> 2) - 1) 
    };
    let mut pmpcfg0: usize = 0;
    // pmp region 0: RW, A=NAPOT, address range {0x1000_1000, 0x1000}, VIRT_VIRTIO
    //                            address range {0x1000_0000, 0x100}, VIRT_UART0
    //                            aligned address range {0x1000_0000, 0x2000}
    pmpcfg0 |= 0b11011; 
    let pmpaddr0 = calc_pmpaddr(0x1000_0000, 0x2000);
    // pmp region 1: RW, A=NAPOT, address range {0x200_0000, 0x1_0000}, VIRT_CLINT 
    pmpcfg0 |= 0b11011 << 8;
    let pmpaddr1 = calc_pmpaddr(0x200_0000, 0x1_0000);
    // pmp region 2: RW, A=NAPOT, address range {0xC00_0000, 0x40_0000}, VIRT_PLIC
    // VIRT_PLIC_SIZE = 0x20_0000 + 0x1000 * harts, thus supports up to 512 harts
    pmpcfg0 |= 0b11011 << 16;
    let pmpaddr2 = calc_pmpaddr(0xC00_0000, 0x40_0000);
    // pmp region 3: RWX, A=NAPOT, address range {0x8000_0000, 0x1000_0000}, VIRT_DRAM
    pmpcfg0 |= 0b11111 << 24;
    let pmpaddr3 = calc_pmpaddr(0x8000_0000, 0x1000_0000);
    unsafe {
        core::arch::asm!("csrw  pmpcfg0, {}",
             "csrw  pmpaddr0, {}",
             "csrw  pmpaddr1, {}",
             "csrw  pmpaddr2, {}",
             "csrw  pmpaddr3, {}",
             "sfence.vma",
             in(reg) pmpcfg0,
             in(reg) pmpaddr0,
             in(reg) pmpaddr1,
             in(reg) pmpaddr2,
             in(reg) pmpaddr3,
        ); 
    }
}

#[naked]
#[link_section = ".text.entry"]
#[export_name = "_start"]
unsafe extern "C" fn entry(_a0: usize, _a1: usize) -> ! {
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
    per_hart_stack_size = const PER_HART_STACK_SIZE,
    stack = sym SBI_STACK,
    rust_main = sym rust_main,
    options(noreturn))
}
