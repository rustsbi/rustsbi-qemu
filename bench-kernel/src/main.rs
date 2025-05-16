#![no_std]
#![no_main]
#![deny(warnings)]

use rcore_console::log;
use riscv::register::*;
use sbi_rt::*;
use uart16550::Uart16550;

#[unsafe(naked)]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start(hartid: usize, device_tree_paddr: usize) -> ! {
    const STACK_SIZE: usize = 16384; // 16 KiB

    #[link_section = ".bss.uninit"]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::naked_asm!(
        "la sp,  {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack      = sym   STACK,
        main       = sym   rust_main,
    )
}

extern "C" fn rust_main(hartid: usize, _dtb_pa: usize) -> ! {
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe {
        let mut ptr = sbss as *mut u64;
        let end = ebss as *mut u64;
        while ptr < end {
            ptr.write_volatile(0);
            ptr = ptr.offset(1);
        }
    }
    // 初始化打印
    unsafe { UART = Uart16550Map(0x1000_0000 as _) };
    rcore_console::init_console(&Console);
    rcore_console::set_log_level(option_env!("LOG"));
    rcore_console::test_log();

    // 测试调用延迟
    let t0 = time::read();

    for _ in 0..100_0000 {
        let _ = sbi_rt::get_spec_version();
    }

    let t1 = time::read();
    log::info!("spec_version duration = {}", t1 - t0);

    // 测试调用延迟
    let t0 = time::read();

    for _ in 0..100_0000 {
        let _ = sbi_rt::get_marchid();
    }

    let t1 = time::read();
    log::info!("marchid duration = {}", t1 - t0);

    // 打开软中断
    unsafe { sie::set_ssoft() };
    // 测试中断响应延迟
    let t0 = time::read();
    for _ in 0..100_0000 {
        unsafe {
            sstatus::set_sie();
            core::arch::asm!(
                "   la    {0}, 1f
                    csrw  stvec, {0}
                    mv    a0, a2
                    mv    a1, zero
                    ecall
                 0: wfi
                    j 0b
                 .align 2
                 1: csrci sip, {ssip}
                ",
                out(reg) _,
                ssip = const 1 << 1,
                in("a7") 0x735049,
                in("a6") 0,
                in("a0") 0,
                in("a1") 0,
                in("a2") 1 << hartid,
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
static mut UART: Uart16550Map = Uart16550Map(core::ptr::null());

pub struct Uart16550Map(*const Uart16550<u8>);

unsafe impl Sync for Uart16550Map {}

impl Uart16550Map {
    #[inline]
    pub fn get(&self) -> &Uart16550<u8> {
        unsafe { &*self.0 }
    }
}

impl rcore_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        unsafe {
            let mut_ref_uart = (*(&raw mut UART)).get();
            mut_ref_uart.write(core::slice::from_ref(&c));
        }
    }

    #[inline]
    fn put_str(&self, s: &str) {
        unsafe {
            let mut_ref_uart = (*(&raw mut UART)).get();
            mut_ref_uart.write(s.as_bytes());
        }
    }
}
