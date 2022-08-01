#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![deny(warnings)]

#[macro_use]
mod console;

use core::arch::asm;
use sbi_testing::sbi;

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
        "   csrw sie, zero
            la   sp,  {stack}
            li   t0,  {stack_size}
            add  sp,  sp, t0
            j    {main}
        ",
        stack_size = const STACK_SIZE,
        stack      = sym   STACK,
        main       = sym   primary_rust_main,
        options(noreturn),
    )
}

extern "C" fn primary_rust_main(hartid: usize, dtb_pa: usize) -> ! {
    zero_bss();

    let BoardInfo {
        smp,
        frequency,
        uart,
    } = BoardInfo::parse(dtb_pa);
    console::init(uart);
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

    sbi_testing::base::test(|case| {
        use sbi_testing::base::Case::*;
        match case {
            NotExist => panic!("Sbi Base Not Exist"),
            Begin => println!("[test-kernel] Testing Base"),
            Pass => println!("[test-kernel] Sbi Base Test Pass"),
            GetSbiSpecVersion(version) => {
                println!("[test-kernel] sbi spec version = {version}");
            }
            GetSbiImplId(Ok(name)) => {
                println!("[test-kernel] sbi impl = {name}");
            }
            GetSbiImplId(Err(unknown)) => {
                println!("[test-kernel] unknown sbi impl = {unknown:#x}");
            }
            GetSbiImplVersion(version) => {
                println!("[test-kernel] sbi impl version = {version:#x}");
            }
            ProbeExtensions(exts) => {
                println!("[test-kernel] sbi extensions = {exts}");
            }
            GetMVendorId(id) => {
                println!("[test-kernel] mvendor id = {id:#x}");
            }
            GetMArchId(id) => {
                println!("[test-kernel] march id = {id:#x}");
            }
            GetMimpId(id) => {
                println!("[test-kernel] mimp id = {id:#x}");
            }
        }
    });
    println!();
    sbi_testing::time::test(frequency, |case| {
        use sbi_testing::time::Case::*;
        match case {
            NotExist => panic!("Sbi TIME Not Exist"),
            Begin => println!("[test-kernel] Testing TIME"),
            Pass => println!("[test-kernel] Sbi TIME Test Pass"),
            Interval { begin: _, end: _ } => {
                println!("[test-kernel] read time register successfuly, set timer +1s");
            }
            TimeDecreased { a, b } => panic!("time decreased: {a} -> {b}"),
            SetTimer => {
                println!("[test-kernel] timer interrupt delegate successfuly");
            }
            UnexpectedTrap(trap) => {
                panic!("expect trap at supervisor timer, but {trap:?} was caught");
            }
        }
    });
    println!();
    sbi_testing::spi::test(hartid, |case| {
        use sbi_testing::spi::Case::*;
        match case {
            NotExist => panic!("Sbi sPI Not Exist"),
            Begin => println!("[test-kernel] Testing sPI"),
            Pass => println!("[test-kernel] Sbi sPI Test Pass"),
            SendIpi => println!("[test-kernel] send ipi successfuly"),
            UnexpectedTrap(trap) => {
                panic!("expect trap at supervisor soft, but {trap:?} was caught")
            }
        }
    });
    sbi_testing::hsm::test(hartid, 0xff, 0, |case| {
        use sbi_testing::hsm::Case::*;
        match case {
            NotExist => panic!("Sbi HSM Not Exist"),
            Begin => println!("[test-kernel] Testing HSM"),
            Pass => println!("[test-kernel] Sbi HSM Test Pass"),
            NoSecondaryHart => println!("[test-kernel] no secondary hart"),
            HartStarted(id) => println!("[test-kernel] hart{id} already started"),
            HartStartFailed { hartid, ret } => panic!("hart {hartid} start failed: {ret:?}"),
        }
    });

    sbi::system_reset(sbi::RESET_TYPE_SHUTDOWN, sbi::RESET_REASON_NO_REASON);
    unreachable!()
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

#[cfg_attr(not(test), panic_handler)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use sbi::{system_reset, RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_SHUTDOWN};

    let (hard_id, pc): (usize, usize);
    unsafe { asm!("mv    {}, tp", out(reg) hard_id) };
    unsafe { asm!("auipc {},  0", out(reg) pc) };
    println!("[test-kernel-panic] hart {hard_id} {info}");
    println!("[test-kernel-panic] pc = {pc:#x}");
    println!("[test-kernel-panic] SBI test FAILED due to panic");
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
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
                matches!(e, E::Misaligned(4) | E::LastCompVersion(16))
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
