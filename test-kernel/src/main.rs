#![no_std]
#![no_main]
#![feature(naked_functions, asm_const)]
#![deny(warnings)]

#[macro_use]
extern crate rcore_console;

use core::{arch::asm, mem::MaybeUninit};
use sbi_testing::sbi;
use spin::Mutex;
use uart_16550::MmioSerialPort;

/// 内核入口。
///
/// # Safety
///
/// 裸函数。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start(hartid: usize, device_tree_paddr: usize) -> ! {
    const STACK_SIZE: usize = 16384; // 16 KiB

    #[link_section = ".bss.uninit"]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    asm!(
        "la sp,  {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack      =   sym STACK,
        main       =   sym rust_main,
        options(noreturn),
    )
}

extern "C" fn rust_main(hartid: usize, dtb_pa: usize) -> ! {
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe {
        let mut ptr = &mut sbss as *mut u64;
        let end = &mut ebss as *mut u64;
        while ptr < end {
            ptr.write_volatile(0);
            ptr = ptr.offset(1);
        }
    }
    let BoardInfo {
        smp,
        frequency,
        uart,
    } = BoardInfo::parse(dtb_pa);
    *UART.lock() = MaybeUninit::new(unsafe { MmioSerialPort::new(uart) });
    rcore_console::init_console(&Console);
    rcore_console::set_log_level(option_env!("LOG"));
    println!(
        r"
 _____         _     _  __                    _
|_   _|__  ___| |_  | |/ /___ _ __ _ __   ___| |
  | |/ _ \/ __| __| | ' // _ \ '__| '_ \ / _ \ |
  | |  __/\__ \ |_  | . \  __/ |  | | | |  __/ |
  |_|\___||___/\__| |_|\_\___|_|  |_| |_|\___|_|
================================================
| boot hart id          | {hartid:20} |
| smp                   | {smp:20} |
| timebase frequency    | {frequency:17} Hz |
| dtb physical address  | {dtb_pa:#20x} |
------------------------------------------------"
    );
    sbi_testing::Testing {
        hartid,
        hart_mask: (1 << smp) - 1,
        hart_mask_base: 0,
        delay: frequency,
    }
    .test();
    sbi::system_reset(sbi::Shutdown, sbi::NoReason);
    unreachable!()
}

#[cfg_attr(not(test), panic_handler)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let (hart_id, pc): (usize, usize);
    unsafe { asm!("mv    {}, tp", out(reg) hart_id) };
    unsafe { asm!("auipc {},  0", out(reg) pc) };
    println!("[test-kernel-panic] hart {hart_id} {info}");
    println!("[test-kernel-panic] pc = {pc:#x}");
    println!("[test-kernel-panic] SBI test FAILED due to panic");
    sbi::system_reset(sbi::Shutdown, sbi::SystemFailure);
    loop {}
}

struct BoardInfo {
    smp: usize,
    frequency: u64,
    uart: usize,
}

impl BoardInfo {
    fn parse(dtb_pa: usize) -> Self {
        use dtb_walker::{Dtb, DtbObj, HeaderError as E, Property, Str, WalkOperation::*};

        let mut ans = Self {
            smp: 0,
            frequency: 0,
            uart: 0,
        };
        unsafe {
            Dtb::from_raw_parts_filtered(dtb_pa as _, |e| {
                matches!(e, E::Misaligned(4) | E::LastCompVersion(_))
            })
        }
        .unwrap()
        .walk(|ctx, obj| match obj {
            DtbObj::SubNode { name } => {
                if ctx.is_root() && (name == Str::from("cpus") || name == Str::from("soc")) {
                    StepInto
                } else if ctx.name() == Str::from("cpus") && name.starts_with("cpu@") {
                    ans.smp += 1;
                    StepOver
                } else if ctx.name() == Str::from("soc") && name.starts_with("uart") {
                    StepInto
                } else {
                    StepOver
                }
            }
            DtbObj::Property(Property::Reg(mut reg)) => {
                if ctx.name().starts_with("uart") {
                    ans.uart = reg.next().unwrap().start;
                }
                StepOut
            }
            DtbObj::Property(Property::General { name, value }) => {
                if ctx.name() == Str::from("cpus") && name == Str::from("timebase-frequency") {
                    ans.frequency = match *value {
                        [a, b, c, d] => u32::from_be_bytes([a, b, c, d]) as _,
                        [a, b, c, d, e, f, g, h] => u64::from_be_bytes([a, b, c, d, e, f, g, h]),
                        _ => unreachable!(),
                    };
                }
                StepOver
            }
            DtbObj::Property(_) => StepOver,
        });
        ans
    }
}

struct Console;
static UART: Mutex<MaybeUninit<MmioSerialPort>> = Mutex::new(MaybeUninit::uninit());

impl rcore_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        unsafe { UART.lock().assume_init_mut() }.send(c);
    }

    #[inline]
    fn put_str(&self, s: &str) {
        let mut uart = UART.lock();
        let uart = unsafe { uart.assume_init_mut() };
        for c in s.bytes() {
            uart.send(c);
        }
    }
}
