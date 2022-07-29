//! A test kernel to test RustSBI function on all platforms

#![feature(naked_functions, asm_sym, asm_const)]
#![no_std]
#![no_main]

use core::arch::asm;
use sbi_testing::sbi;

#[macro_use]
mod console;

mod constants {
    pub(crate) const LEN_PAGE: usize = 4096; // 4KiB
    pub(crate) const PER_HART_STACK_SIZE: usize = 4 * LEN_PAGE; // 16KiB
    pub(crate) const MAX_HART_NUMBER: usize = 8; // assume 8 cores in QEMU
    pub(crate) const STACK_SIZE: usize = PER_HART_STACK_SIZE * MAX_HART_NUMBER;
}

use constants::*;

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

/// 内核入口。
///
/// # Safety
///
/// 裸函数。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start(hartid: usize, device_tree_paddr: usize) -> ! {
    asm!(
        "csrw sie, zero",      // 关中断
        "call {select_stack}", // 设置启动栈
        "j    {main}",         // 进入 rust
        select_stack = sym select_stack,
        main = sym primary_rust_main,
        options(noreturn)
    )
}

extern "C" fn primary_rust_main(hartid: usize, dtb_pa: usize) -> ! {
    zero_bss();

    let BoardInfo { smp, uart } = parse_smp(dtb_pa);
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
| dtb physical address  | {dtb_pa:#20x} |
------------------------------------------------"
    );

    use sbi_testing::{
        base::NotExist, hsm::Case as Hsm, spi::SendIpi, Case, Extension as Ext, Testing,
    };
    Testing {
        hart_id: hartid,
        hart_mask: 0xff,
        hart_mask_base: 0,
        delay: 30_000_000,
    }
    .test(|case| match case {
        Case::Begin(ext) => {
            match ext {
                Ext::Base => println!("[test-kernel] Testing Base"),
                Ext::Time => println!("[test-kernel] Testing TIME"),
                Ext::Spi => println!("[test-kernel] Testing sPI"),
                Ext::Hsm => println!("[test-kernel] Testing HSM"),
            }
            true
        }
        Case::End(_) => true,
        Case::Base(case) => {
            use sbi_testing::base::Case::*;
            match case {
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
            true
        }
        Case::BaseFatel(NotExist) => panic!("sbi base not exist"),
        Case::Time(case) => {
            use sbi_testing::time::Case::*;
            match case {
                Interval { begin: _, end: _ } => {
                    println!("[test-kernel] read time register successfuly, set timer +3s");
                }
                SetTimer => {
                    println!("[test-kernel] timer interrupt delegate successfuly");
                }
            }
            true
        }
        Case::TimeFatel(fatel) => {
            use sbi_testing::time::Fatel::*;
            match fatel {
                NotExist => panic!("sbi time not exist"),
                TimeDecreased { a, b } => panic!("time decreased: {a} -> {b}"),
                UnexpectedTrap(trap) => {
                    panic!("expect trap at supervisor timer, but {trap:?} was caught");
                }
            }
        }
        Case::Spi(SendIpi) => {
            println!("[test-kernel] send ipi successfuly");
            true
        }
        Case::SpiFatel(fatel) => {
            use sbi_testing::spi::Fatel::*;
            match fatel {
                NotExist => panic!("sbi spi not exist"),
                UnexpectedTrap(trap) => {
                    panic!("expect trap at supervisor soft, but {trap:?} was caught");
                }
            }
        }
        Case::Hsm(Hsm::HartStarted(id)) => {
            println!("[test-kernel] hart{id} already started");
            true
        }
        Case::HsmFatel(fatel) => {
            use sbi_testing::hsm::Fatel::*;
            match fatel {
                NotExist => {
                    println!("[test-kernel] sbi hsm not exist");
                    true
                }
                NoSecondaryHart => panic!("no secondary hart"),
                HartStartFailed { hartid, ret } => panic!("hart {hartid} start failed: {ret:?}"),
            }
        }
    });

    sbi::system_reset(sbi::RESET_TYPE_SHUTDOWN, sbi::RESET_REASON_NO_REASON);
    unreachable!()
}

/// 根据硬件线程号设置启动栈。
///
/// # Safety
///
/// 裸函数。
#[naked]
unsafe extern "C" fn select_stack(hartid: usize) {
    #[link_section = ".bss.uninit"]
    static mut BOOT_STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    asm!("
           mv   tp, a0
           addi t0, a0,  1
           la   sp, {stack}
           li   t1, {len_per_hart}
        1: add  sp, sp, t1
           addi t0, t0, -1
           bnez t0, 1b
           ret
        ",
        stack = sym BOOT_STACK,
        len_per_hart = const PER_HART_STACK_SIZE,
        options(noreturn)
    )
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

struct BoardInfo {
    smp: usize,
    uart: usize,
}

fn parse_smp(dtb_pa: usize) -> BoardInfo {
    use dtb_walker::{Dtb, DtbObj, HeaderError as E, Property, Str, WalkOperation::*};

    let mut ans = BoardInfo { smp: 0, uart: 0 };
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
        DtbObj::Property(_) => StepOver,
    });
    ans
}
