//! A test kernel to test RustSBI function on all platforms

#![feature(naked_functions, asm_sym, asm_const)]
#![no_std]
#![no_main]

use core::arch::asm;
use riscv::register::{
    scause::{Interrupt, Trap},
    stvec::{self, TrapMode},
};
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

    use sbi_testing::{base::NotExist, spi::SendIpi, Case, Extension as Ext};
    let _ = sbi_testing::test(hartid, 30_000_000, |case| match case {
        Case::Begin(ext) => {
            match ext {
                Ext::Base => println!("[test-kernel] Testing Base"),
                Ext::Time => println!("[test-kernel] Testing TIME"),
                Ext::Spi => println!("[test-kernel] Testing sPI"),
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
    });

    unsafe { stvec::write(start_trap as usize, TrapMode::Direct) };
    test_hsm(hartid, smp);

    sbi::system_reset(sbi::RESET_TYPE_SHUTDOWN, sbi::RESET_REASON_NO_REASON);
    unreachable!()
}

extern "C" fn rust_trap_exception(trap_frame: &mut TrapFrame) {
    match riscv::register::scause::read().cause() {
        Trap::Interrupt(Interrupt::SupervisorSoft) => unsafe { core::arch::asm!("csrw sip, zero") },
        cause => panic!(
            "[test-kernel] SBI test FAILED due to unexpected trap {cause:?} on {}",
            trap_frame.tp
        ),
    }
}

#[cfg(target_pointer_width = "64")]
macro_rules! define_store_load {
    () => {
        ".altmacro
        .macro STORE reg, offset
            sd  \\reg, \\offset* {REGBYTES} (sp)
        .endm
        .macro LOAD reg, offset
            ld  \\reg, \\offset* {REGBYTES} (sp)
        .endm"
    };
}

#[cfg(target_pointer_width = "32")]
macro_rules! define_store_load {
    () => {
        ".altmacro
        .macro STORE reg, offset
            sw  \\reg, \\offset* {REGBYTES} (sp)
        .endm
        .macro LOAD reg, offset
            lw  \\reg, \\offset* {REGBYTES} (sp)
        .endm"
    };
}

#[naked]
#[link_section = ".text.trap_handler"]
unsafe extern "C" fn start_trap() {
    asm!(define_store_load!(), "
    addi    sp, sp, -17 * {REGBYTES}
    STORE   ra, 0
    STORE   t0, 1
    STORE   t1, 2
    STORE   t2, 3
    STORE   t3, 4
    STORE   t4, 5
    STORE   t5, 6
    STORE   t6, 7
    STORE   a0, 8
    STORE   a1, 9
    STORE   a2, 10
    STORE   a3, 11
    STORE   a4, 12
    STORE   a5, 13
    STORE   a6, 14
    STORE   a7, 15
    STORE   tp, 16
    mv      a0, sp
    call    {rust_trap_exception}
    LOAD    ra, 0
    LOAD    t0, 1
    LOAD    t1, 2
    LOAD    t2, 3
    LOAD    t3, 4
    LOAD    t4, 5
    LOAD    t5, 6
    LOAD    t6, 7
    LOAD    a0, 8
    LOAD    a1, 9
    LOAD    a2, 10
    LOAD    a3, 11
    LOAD    a4, 12
    LOAD    a5, 13
    LOAD    a6, 14
    LOAD    a7, 15
    LOAD    tp, 16
    addi    sp, sp, 17 * {REGBYTES}
    sret
    ",
    REGBYTES = const core::mem::size_of::<usize>(),
    rust_trap_exception = sym rust_trap_exception,
    options(noreturn))
}

#[repr(C)]
struct TrapFrame {
    ra: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    tp: usize,
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

/// 所有副核：启动 -> 不可恢复休眠 -> 唤醒 -> 可恢复休眠 -> 唤醒 -> 关闭。
pub(crate) fn test_hsm(hartid: usize, smp: usize) {
    use sbi::SbiRet;
    use spin::{Barrier, Once};

    const SUSPENDED: SbiRet = SbiRet {
        error: sbi::RET_SUCCESS,
        value: sbi::HART_STATE_SUSPENDED,
    };
    const STOPPED: SbiRet = SbiRet {
        error: sbi::RET_SUCCESS,
        value: sbi::HART_STATE_STOPPED,
    };

    static STARTED: Once<Barrier> = Once::new();
    static RESUMED: Once<Barrier> = Once::new();

    #[naked]
    unsafe extern "C" fn test_entry(hartid: usize, main: usize) -> ! {
        core::arch:: asm!(
            "csrw sie, zero",      // 关中断
            "call {select_stack}", // 设置启动栈
            "jr   a1",             // 进入 rust
            select_stack = sym crate::select_stack,
            options(noreturn)
        )
    }

    extern "C" fn start_rust_main(hart_id: usize) -> ! {
        STARTED.wait().wait();
        let ret = sbi::hart_suspend(
            sbi::HART_SUSPEND_TYPE_NON_RETENTIVE,
            test_entry as _,
            resume_rust_main as _,
        );
        unreachable!("suspend [{hart_id}] but {ret:?}");
    }

    extern "C" fn resume_rust_main(hart_id: usize) -> ! {
        RESUMED.wait().wait();
        let ret = sbi::hart_suspend(sbi::HART_SUSPEND_TYPE_RETENTIVE, 0, 0);
        assert_eq!(sbi::RET_SUCCESS, ret.error);
        let ret = sbi::hart_stop();
        unreachable!("stop [{hart_id}] but {ret:?}");
    }

    println!(
        "
[test-kernel] Testing hsm: start, stop, suspend and resume"
    );

    // 启动副核
    let started = STARTED.call_once(|| Barrier::new(smp));
    let resumed = RESUMED.call_once(|| Barrier::new(smp));
    for id in 0..smp {
        if id != hartid {
            println!("[test-kernel] Hart{id} is booting...");
            let ret = sbi::hart_start(id, test_entry as _, start_rust_main as _);
            if ret.error != sbi::RET_SUCCESS {
                panic!("[test-kernel] Start hart{id} failed: {ret:?}");
            }
        } else {
            println!("[test-kernel] Hart{id} is the primary hart.");
        }
    }
    // 等待副核启动完成
    started.wait();
    print!("[test-kernel] All harts boot successfully!\n");
    // 等待副核休眠（不可恢复）
    for id in 0..smp {
        if id != hartid {
            while sbi::hart_get_status(id) != SUSPENDED {
                core::hint::spin_loop();
            }
            println!("[test-kernel] Hart{id} suspended.");
        } else {
            println!("[test-kernel] Hart{id} is the primary hart.");
        }
    }
    // 全部唤醒
    sbi::send_ipi(0, -1isize as usize);
    // 等待副核恢复完成
    resumed.wait();
    print!("[test-kernel] All harts resume successfully!\n");
    for id in 0..smp {
        if id != hartid {
            // 等待副核休眠
            while sbi::hart_get_status(id) != SUSPENDED {
                core::hint::spin_loop();
            }
            print!("[test-kernel] Hart{id} suspended, ");
            // 单独唤醒
            sbi::send_ipi(1usize << id, 0);
            // 等待副核关闭
            while sbi::hart_get_status(id) != STOPPED {
                core::hint::spin_loop();
            }
            println!("then stopped.");
        } else {
            println!("[test-kernel] Hart{id} is the primary hart.");
        }
    }
    println!("[test-kernel] All harts stop successfully!");
}
