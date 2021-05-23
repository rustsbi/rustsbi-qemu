#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(alloc_error_handler)]
#![feature(llvm_asm)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(generator_trait)]
#![feature(default_alloc_error_handler)]

mod clint;
mod ns16550a;
mod test_device;
mod execute;

use core::panic::PanicInfo;
use buddy_system_allocator::LockedHeap;

use rustsbi::{print, println};

use riscv::register::{
    mcause::{self, Exception, Interrupt, Trap},
    medeleg, mepc, mhartid, mideleg, mie, mip, misa::{self, MXL},
    mstatus::{self, MPP, SPP},
    mtval,
    mtvec::{self, TrapMode},
    stvec, scause, stval, sepc,
};

const SBI_HEAP_SIZE: usize = 64 * 1024;
static mut HEAP_SPACE: [u8; SBI_HEAP_SIZE] = [0; SBI_HEAP_SIZE];
#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(info: &PanicInfo) -> ! {
    let hart_id = mhartid::read();
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
    execute::init();
    if hartid == 0 {
        unsafe {
            HEAP.lock().init(
                HEAP_SPACE.as_ptr() as usize, SBI_HEAP_SIZE
            )
        }
        let serial = ns16550a::Ns16550a::new(0x10000000, 0, 11_059_200, 115200);
        use rustsbi::legacy_stdio::init_legacy_stdio_embedded_hal;
        init_legacy_stdio_embedded_hal(serial);

    }
    println!("Hello world!");
    loop {}
}

const BOOT_STACK_SIZE: usize = 4096 * 4 * 8;

#[link_section = ".bss.stack"]
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
