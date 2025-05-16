#![no_std]
#![no_main]
#![deny(warnings)]

mod clint;
mod dbcn;
mod device_tree;
mod hart_csr_utils;
mod qemu_test;
mod riscv_spec;
mod trap_stack;
mod trap_vec;
mod uart16550;

mod constants {
    /// 特权软件入口。
    pub(crate) const SUPERVISOR_ENTRY: usize = 0x8020_0000;
    /// 每个硬件线程设置 16KiB 栈空间。
    pub(crate) const LEN_STACK_PER_HART: usize = 16 * 1024;
    /// qemu-virt 最多 8 个硬件线程。
    pub(crate) const NUM_HART_MAX: usize = 8;
}

#[macro_use]
extern crate rcore_console;

use constants::*;
use core::{
    arch::{asm, naked_asm}, mem::MaybeUninit, sync::atomic::{AtomicBool, Ordering}
};
use device_tree::BoardInfo;
use fast_trap::{FastContext, FastResult};
use riscv_spec::*;
use rustsbi::{RustSBI, SbiRet};
use spin::{Mutex, Once};
use trap_stack::{local_hsm, local_remote_hsm, remote_hsm};
use trap_vec::trap_vec;

/// 入口。
///
/// # Safety
///
/// 裸函数。
#[unsafe(naked)]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "   call {locate_stack}
            call {rust_main}
            j    {trap}
        ",
        locate_stack = sym trap_stack::locate,
        rust_main    = sym rust_main,
        trap         = sym trap_vec,
    )
}

/// rust 入口。
extern "C" fn rust_main(hartid: usize, opaque: usize) {
    static GENESIS: AtomicBool = AtomicBool::new(true);
    static BOARD_INFO: Once<BoardInfo> = Once::new();

    // 全局初始化过程
    if GENESIS.swap(false, Ordering::AcqRel) {
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
        // 解析设备树
        let board_info = BOARD_INFO.call_once(|| device_tree::parse(opaque));
        // 初始化外设
        uart16550::init(board_info.uart.start);
        rcore_console::init_console(&Console);
        rcore_console::set_log_level(option_env!("LOG"));
        clint::init(board_info.clint.start);
        qemu_test::init(board_info.test.start);
        dbcn::init(SUPERVISOR_ENTRY..board_info.mem.end);
        // 打印启动信息
        print!(
            "\
[rustsbi] RustSBI version {ver_sbi}, adapting to RISC-V SBI v2.0.0
{logo}
[rustsbi] Implementation     : RustSBI-QEMU Version {ver_impl}
[rustsbi] Platform Name      : {model}
[rustsbi] Platform SMP       : {smp}
[rustsbi] Platform Memory    : {mem:#x?}
[rustsbi] Boot HART          : {hartid}
[rustsbi] Device Tree Region : {dtb:#x?}
[rustsbi] Firmware Address   : {firmware:#x}
[rustsbi] Supervisor Address : {SUPERVISOR_ENTRY:#x}
",
            ver_sbi = rustsbi::VERSION,
            logo = rustsbi::LOGO,
            ver_impl = env!("CARGO_PKG_VERSION"),
            model = board_info.model,
            smp = board_info.smp,
            mem = board_info.mem,
            dtb = board_info.dtb,
            firmware = _start as usize,
        );
        // 初始化 SBI
        let sbi = SBI.lock().as_mut_ptr();
        unsafe {
            *sbi = FixedRustSBI {
                clint: &clint::Clint,
                hsm: Hsm,
                reset: qemu_test::get(),
                dbcn: dbcn::get(),
            };
        }
        // 设置并打印 pmp
        set_pmp(board_info);
        hart_csr_utils::print_pmps();
        // 设置陷入栈
        trap_stack::prepare_for_trap();
        // 设置内核入口
        local_remote_hsm().start(Supervisor {
            start_addr: SUPERVISOR_ENTRY,
            opaque,
        });
    } else {
        // 设置 pmp
        set_pmp(BOARD_INFO.wait());
        // 设置陷入栈
        trap_stack::prepare_for_trap();
    }
    // 清理 clint
    clint::clear();
    // 准备启动调度
    unsafe {
        asm!("csrw mideleg,    {}", in(reg) !0);
        asm!("csrw medeleg,    {}", in(reg) !0);
        asm!("csrw mcounteren, {}", in(reg) !0);
        use riscv::register::{medeleg, mtvec};
        medeleg::clear_supervisor_env_call();
        medeleg::clear_machine_env_call();
        mtvec::write(trap_vec as _, mtvec::TrapMode::Vectored);
    }
}

#[inline(always)]
fn hart_id() -> usize {
    riscv::register::mhartid::read()
}

/// 设置 PMP。
fn set_pmp(board_info: &BoardInfo) {
    use riscv::register::*;
    let mem = &board_info.mem;
    unsafe {
        pmpcfg0::set_pmp(0, Range::OFF, Permission::NONE, false);
        pmpaddr0::write(0);
        // 外设
        pmpcfg0::set_pmp(1, Range::TOR, Permission::RW, false);
        pmpaddr1::write(mem.start >> 2);
        // SBI
        pmpcfg0::set_pmp(2, Range::TOR, Permission::NONE, false);
        pmpaddr2::write(SUPERVISOR_ENTRY >> 2);
        // 主存
        pmpcfg0::set_pmp(3, Range::TOR, Permission::RWX, false);
        pmpaddr3::write(mem.end >> 2);
        // 其他
        pmpcfg0::set_pmp(4, Range::TOR, Permission::RW, false);
        pmpaddr4::write(1 << (usize::BITS - 1));
    }
}

extern "C" fn fast_handler(
    mut ctx: FastContext,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
) -> FastResult {
    use riscv::register::{
        mcause::{self, Exception as E, Trap as T},
        mtval, satp, sstatus,
    };

    #[inline]
    fn boot(mut ctx: FastContext, start_addr: usize, opaque: usize) -> FastResult {
        unsafe {
            sstatus::clear_sie();
            satp::write(0);
        }
        ctx.regs().a[0] = hart_id();
        ctx.regs().a[1] = opaque;
        ctx.regs().pc = start_addr;
        ctx.call(2)
    }
    loop {
        match local_hsm().start() {
            Ok(supervisor) => {
                mstatus::update(|bits| {
                    *bits &= !mstatus::MPP;
                    *bits |= mstatus::MPIE | mstatus::MPP_SUPERVISOR;
                });
                mie::write(mie::MSIE | mie::MTIE);
                break boot(ctx, supervisor.start_addr, supervisor.opaque);
            }
            Err(rustsbi::spec::hsm::HART_STOP) => {
                mie::write(mie::MSIE);
                unsafe { riscv::asm::wfi() };
                clint::clear_msip();
            }
            _ => match mcause::read().cause() {
                // SBI call
                T::Exception(E::SupervisorEnvCall) => {
                    use sbi_spec::{base, hsm, legacy};
                    let mut ret = unsafe {
                        let sbi = SBI.lock().as_mut_ptr();
                        (*sbi).handle_ecall(
                            a7,
                            a6,
                            [ctx.a0(), a1, a2, a3, a4, a5],
                        )
                    };
                    if ret.is_ok() {
                        match (a7, a6) {
                            // 关闭
                            (hsm::EID_HSM, hsm::HART_STOP) => continue,
                            // 不可恢复挂起
                            (hsm::EID_HSM, hsm::HART_SUSPEND)
                                if matches!(ctx.a0() as u32, hsm::suspend_type::NON_RETENTIVE) =>
                            {
                                break boot(ctx, a1, a2);
                            }
                            // legacy console 探测
                            (base::EID_BASE, base::PROBE_EXTENSION)
                                if matches!(
                                    ctx.a0(),
                                    legacy::LEGACY_CONSOLE_PUTCHAR | legacy::LEGACY_CONSOLE_GETCHAR
                                ) =>
                            {
                                ret.value = 1;
                            }
                            _ => {}
                        }
                    } else {
                        match a7 {
                            legacy::LEGACY_CONSOLE_PUTCHAR => {
                                print!("{}", ctx.a0() as u8 as char);
                                ret.error = 0;
                                ret.value = a1;
                            }
                            legacy::LEGACY_CONSOLE_GETCHAR => {
                                let mut c = 0u8;
                                let uart = uart16550::UART.lock();
                                loop {
                                    if uart.get().read(core::slice::from_mut(&mut c)) == 1 {
                                        ret.error = c as _;
                                        ret.value = a1;
                                        break;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    ctx.regs().a = [ret.error, ret.value, a2, a3, a4, a5, a6, a7];
                    mepc::next();
                    break ctx.restore();
                }
                // 其他陷入
                trap => {
                    println!(
                        "
-----------------------------
> trap:    {trap:?}
> mstatus: {:#018x}
> mepc:    {:#018x}
> mtval:   {:#018x}
-----------------------------
            ",
                        mstatus::read(),
                        mepc::read(),
                        mtval::read()
                    );
                    panic!("stopped with unsupported trap")
                }
            },
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use rustsbi::{
        spec::srst::{RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_SHUTDOWN},
        Reset,
    };
    // 输出的信息大概是“[rustsbi-panic] hart 0 panicked at ...”
    println!("[rustsbi-panic] hart {} {info}", hart_id());
    println!("[rustsbi-panic] system shutdown scheduled due to RustSBI panic");
    qemu_test::get().system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
    unreachable!()
}

/// 特权软件信息。
#[derive(Debug)]
struct Supervisor {
    start_addr: usize,
    opaque: usize,
}

struct Console;

impl rcore_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        let uart = uart16550::UART.lock();
        while uart.get().write(&[c]) == 0 {
            core::hint::spin_loop();
        }
    }

    #[inline]
    fn put_str(&self, s: &str) {
        let uart = uart16550::UART.lock();
        let mut bytes = s.as_bytes();
        while !bytes.is_empty() {
            let count = uart.get().write(bytes);
            bytes = &bytes[count..];
        }
    }
}

static SBI: Mutex<MaybeUninit<FixedRustSBI>> = Mutex::new(MaybeUninit::uninit());

#[derive(RustSBI)]
struct FixedRustSBI<'a> {
    #[rustsbi(ipi, timer)]
    clint: &'a clint::Clint,
    hsm: Hsm,
    reset: &'a qemu_test::QemuTest,
    dbcn: &'a dbcn::DBCN,
}

struct Hsm;

impl rustsbi::Hsm for Hsm {
    fn hart_start(&self, hartid: usize, start_addr: usize, opaque: usize) -> SbiRet {
        match remote_hsm(hartid) {
            Some(remote) => {
                if remote.start(Supervisor { start_addr, opaque }) {
                    clint::set_msip(hartid);
                    SbiRet::success(0)
                } else {
                    SbiRet::already_started()
                }
            }
            None => SbiRet::invalid_param(),
        }
    }

    #[inline]
    fn hart_stop(&self) -> SbiRet {
        local_hsm().stop();
        SbiRet::success(0)
    }

    #[inline]
    fn hart_get_status(&self, hartid: usize) -> SbiRet {
        match remote_hsm(hartid) {
            Some(remote) => SbiRet::success(remote.sbi_get_status()),
            None => SbiRet::invalid_param(),
        }
    }

    fn hart_suspend(&self, suspend_type: u32, _resume_addr: usize, _opaque: usize) -> SbiRet {
        use rustsbi::spec::hsm::suspend_type::{NON_RETENTIVE, RETENTIVE};
        if matches!(suspend_type, NON_RETENTIVE | RETENTIVE) {
            local_hsm().suspend();
            unsafe { riscv::asm::wfi() };
            local_hsm().resume();
            SbiRet::success(0)
        } else {
            SbiRet::not_supported()
        }
    }
}
