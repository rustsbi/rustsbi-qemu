#![no_std]
#![no_main]
#![feature(naked_functions, asm_const)]
#![deny(warnings)]

use core::mem::MaybeUninit;

use console::log;
use riscv::register::*;
use sbi_rt::*;
use uart_16550::MmioSerialPort;

#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start(hartid: usize, device_tree_paddr: usize) -> ! {
    const STACK_SIZE: usize = 16384; // 16 KiB

    #[link_section = ".bss.uninit"]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::asm!(
        "la sp,  {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack      = sym   STACK,
        main       = sym   rust_main,
        options(noreturn),
    )
}

extern "C" fn rust_main(_hartid: usize, _dtb_pa: usize) -> ! {
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
    unsafe { UART = MaybeUninit::new(MmioSerialPort::new(0x1000_0000)) };
    console::init_console(&Console);
    console::set_log_level(option_env!("LOG"));
    console::test_log();

    let t0 = time::read();

    for _ in 0..0xffff {
        let _ = sbi_rt::get_marchid();
    }

    let t1 = time::read();

    log::info!("{}", t1 - t0);
    system_reset(Shutdown, NoReason);
    unreachable!()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{info}");
    system_reset(Shutdown, SystemFailure);
    loop {}
}

struct Console;
static mut UART: MaybeUninit<MmioSerialPort> = MaybeUninit::uninit();

impl console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        unsafe { UART.assume_init_mut().send(c) }
    }
}
