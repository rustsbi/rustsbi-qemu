pub(crate) fn base_extension() {
    println!(
        "
[test-kernel] Testing base extension"
    );
    let base_version = sbi::probe_extension(sbi::EID_BASE);
    if base_version == 0 {
        panic!(
            "\
[test-kernel] no base extension probed; SBI call returned value '0'
[test-kernel] this SBI implementation may only have legacy extension implemented"
        );
    }

    let spec_version = sbi::get_spec_version();
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
        impl_id = sbi::get_sbi_impl_id(),
        impl_version = sbi::get_sbi_impl_version(),
        mvendorid = sbi::get_mvendorid(),
        marchid = sbi::get_marchid(),
        mimpid = sbi::get_mimpid(),
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

pub(crate) fn trap_delegate(hartid: usize) {
    use crate::EXPECTED;
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

/// 操作一个静态的原子变量。
/// 现在除启动核外，所有核都处于 STOPPED 状态，
/// 主核在 [`sbi::hart_start`] 之前给原子变量 +1。
/// 核启动后，会自动给原子变量 -1。
/// 所有核成功启动则原子变量值归零。
/// 所有核一起等待原子变量归零，然后副核调用 [`sbi::hart_stop`] 关闭。
/// 主核等待所有副核关闭，然后退出。
pub(crate) fn start_stop_harts(hartid: usize, smp: usize) {
    use spin::{Barrier, Once};
    static BARRIER: Once<Barrier> = Once::new();

    #[naked]
    unsafe extern "C" fn test_start_entry(hartid: usize) -> ! {
        core::arch:: asm!(
            "csrw sie, zero",      // 关中断
            "call {select_stack}", // 设置启动栈
            "j    {main}",         // 进入 rust
            select_stack = sym crate::select_stack,
            main = sym secondary_rust_main,
            options(noreturn)
        )
    }

    extern "C" fn secondary_rust_main(hart_id: usize) -> ! {
        BARRIER.wait().wait();
        let ret = sbi::hart_stop();
        unreachable!("stop [{hart_id}] but {ret:?}");
    }

    println!(
        "
[test-kernel] Testing start harts"
    );

    // 启动副核
    let barrier = BARRIER.call_once(|| Barrier::new(smp));
    for id in 0..smp {
        if id != hartid {
            println!("[test-kernel] Hart{id} is booting...");
            let ret = sbi::hart_start(id, test_start_entry as usize, 0);
            if ret.error != sbi::RET_SUCCESS {
                panic!("[test-kernel] Start hart{id} failed: {ret:?}");
            }
        } else {
            println!("[test-kernel] Hart{id} is the primary hart.");
        }
    }
    // 等待副核启动完成
    barrier.wait();
    print!("[test-kernel] All harts boot successfully!\n");
    // 等待副核关闭
    for id in 0..smp {
        const STOPPED: sbi::SbiRet = sbi::SbiRet {
            error: sbi::RET_SUCCESS,
            value: sbi::HART_STATE_STOPPED,
        };
        if id != hartid {
            while sbi::hart_get_status(id) != STOPPED {
                core::hint::spin_loop();
            }
            println!("[test-kernel] Hart{id} stopped.");
        } else {
            println!("[test-kernel] Hart{id} is the primary hart.");
        }
    }
    println!("[test-kernel] All harts stop successfully!");
}
