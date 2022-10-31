#![no_std]
#![no_main]
#![feature(naked_functions, asm_const)]
#![deny(warnings)]

use core::mem::MaybeUninit;
use rcore_console::log;
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

extern "C" fn rust_main(hartid: usize, _dtb_pa: usize) -> ! {
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
    // 初始化打印
    unsafe { UART = MaybeUninit::new(MmioSerialPort::new(0x1000_0000)) };
    rcore_console::init_console(&Console);
    rcore_console::set_log_level(option_env!("LOG"));
    rcore_console::test_log();
    // 打开软中断
    unsafe {
        sie::set_ssoft();
        sstatus::set_sie();
    };
    // 测试调用延迟
    let t0 = time::read();

    for _ in 0..0xffff {
        let _ = sbi_rt::get_marchid();
    }

    let t1 = time::read();
    log::info!("marchid duration = {}", t1 - t0);
    // 测试中断响应延迟
    let t0 = time::read();

    for _ in 0..0xffff {
        unsafe {
            core::arch::asm!(
                "   la   {0}, 1f
                    csrw stvec, {0}
                    ecall
                    wfi
                .align 2
                1:
                ",
                out(reg) _,
                in("a7") 0x735049,
                in("a6") 0,
                in("a0") 1 << hartid,
                in("a1") 0,
                options(nomem),
            );
        }
    }

    let t1 = time::read();
    log::info!("ipi duration = {}", t1 - t0);

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

impl rcore_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        unsafe { UART.assume_init_mut() }.send(c);
    }
}
