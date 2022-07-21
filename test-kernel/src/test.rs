use crate::EXPECTED;

pub(crate) fn base_extension() {
    println!(
        "
[test-kernel] Testing base extension"
    );
    let base_version = sbi_rt::probe_extension(sbi_rt::EID_BASE);
    if base_version == 0 {
        panic!(
            "\
[test-kernel] no base extension probed; SBI call returned value '0'
[test-kernel] this SBI implementation may only have legacy extension implemented"
        );
    }

    let spec_version = sbi_rt::get_spec_version();
    println!(
        "\
[test-kernel] Base extension version: {base_version:x}
[test-kernel] SBI specification version: {major}.{minor}
[test-kernel] SBI implementation Id: {impl_id:x}
[test-kernel] SBI implementation version: {impl_version:x}
[test-kernel] Device mvendorid: {mvendorid:x}
[test-kernel] Device marchid: {marchid:x}
[test-kernel] Device mimpid: {mimpid:x}",
        major = (spec_version >> 24) & 0x7F,
        minor = spec_version & 0xFFFFFF,
        impl_id = sbi_rt::get_sbi_impl_id(),
        impl_version = sbi_rt::get_sbi_impl_version(),
        mvendorid = sbi_rt::get_mvendorid(),
        marchid = sbi_rt::get_marchid(),
        mimpid = sbi_rt::get_mimpid(),
    );
}

pub(crate) fn sbi_ins_emulation() {
    use riscv::register::time;

    let time_start = time::read64();
    println!(
        "
[test-kernel] Testing SBI instruction emulation
[test-kernel] Current time: {time_start}"
    );
    let time_end = time::read64();
    if time_end > time_start {
        println!(
            "\
[test-kernel] Time after operation: {time_end}"
        );
    } else {
        panic!(
            "\
[test-kernel] SBI test FAILED due to incorrect time counter"
        );
    }
}

pub(crate) fn trap_execption_delegate(hartid: usize) {
    use core::arch::asm;
    use riscv::register::scause::{Exception, Trap};

    println!(
        "
[test-kernel] Testing trap delegate
[test-kernel] Trigger illegal exception"
    );

    unsafe {
        // expect a trap from {hartid}
        EXPECTED[hartid] = Some(Trap::Exception(Exception::IllegalInstruction));
        // mcycle cannot be written, this is always a 4-byte illegal instruction
        asm!("csrw mcycle, x0");
    }
    println!(
        "\
[test-kernel] Illegal exception delegate success"
    );
}

pub(crate) fn trap_interrupt_delegate(hartid: usize) {
    use core::arch::asm;
    use riscv::register::{
        scause::{Interrupt, Trap},
        sie, time,
    };

    println!(
        "
[test-kernel] Testing trap delegate
[test-kernel] Set timer +1s"
    );
    unsafe {
        sie::set_stimer();
        EXPECTED[hartid] = Some(Trap::Interrupt(Interrupt::SupervisorTimer));
    }
    sbi_rt::set_timer(time::read64() + (10 << 20));

    unsafe { riscv::asm::wfi() };
    println!(
        "\
[test-kernel] Timer interrupt delegate success"
    );
}

/// 所有副核：启动 -> 不可恢复休眠 -> 唤醒 -> 可恢复休眠 -> 唤醒 -> 关闭。
pub(crate) fn hsm(hartid: usize, smp: usize) {
    use sbi_rt::SbiRet;
    const SUSPENDED: SbiRet = SbiRet {
        error: sbi_rt::RET_SUCCESS,
        value: sbi_rt::HART_STATE_SUSPENDED,
    };
    const STOPPED: SbiRet = SbiRet {
        error: sbi_rt::RET_SUCCESS,
        value: sbi_rt::HART_STATE_STOPPED,
    };

    use spin::{Barrier, Once};
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
        let ret = sbi_rt::hart_suspend(
            sbi_rt::HART_SUSPEND_TYPE_NON_RETENTIVE,
            test_entry as _,
            resume_rust_main as _,
        );
        unreachable!("suspend [{hart_id}] but {ret:?}");
    }

    extern "C" fn resume_rust_main(hart_id: usize) -> ! {
        RESUMED.wait().wait();
        let ret = sbi_rt::hart_suspend(sbi_rt::HART_SUSPEND_TYPE_RETENTIVE, 0, 0);
        assert_eq!(sbi_rt::RET_SUCCESS, ret.error);
        let ret = sbi_rt::hart_stop();
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
            let ret = sbi_rt::hart_start(id, test_entry as _, start_rust_main as _);
            if ret.error != sbi_rt::RET_SUCCESS {
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
            while sbi_rt::hart_get_status(id) != SUSPENDED {
                core::hint::spin_loop();
            }
            println!("[test-kernel] Hart{id} suspended.");
        } else {
            println!("[test-kernel] Hart{id} is the primary hart.");
        }
    }
    // 全部唤醒
    sbi_rt::send_ipi(0, -1isize as usize);
    // 等待副核恢复完成
    resumed.wait();
    print!("[test-kernel] All harts resume successfully!\n");
    for id in 0..smp {
        if id != hartid {
            // 等待副核休眠
            while sbi_rt::hart_get_status(id) != SUSPENDED {
                core::hint::spin_loop();
            }
            print!("[test-kernel] Hart{id} suspended, ");
            // 单独唤醒
            sbi_rt::send_ipi(1usize << id, 0);
            // 等待副核关闭
            while sbi_rt::hart_get_status(id) != STOPPED {
                core::hint::spin_loop();
            }
            println!("then stopped.");
        } else {
            println!("[test-kernel] Hart{id} is the primary hart.");
        }
    }
    println!("[test-kernel] All harts stop successfully!");
}
