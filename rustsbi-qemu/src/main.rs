#![no_std]
#![no_main]
#![feature(naked_functions, asm_const)]
#![deny(warnings)]

mod clint;
mod device_tree;
mod hart_csr_utils;
mod qemu_test;
mod riscv_spec;
mod trap_vec;

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
    arch::asm,
    convert::Infallible,
    mem::{forget, size_of, MaybeUninit},
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};
use device_tree::BoardInfo;
use fast_trap::{FastContext, FastResult, FlowContext, FreeTrapStack, TrapStackBlock};
use hsm_cell::HsmCell;
use riscv_spec::*;
use rustsbi::RustSBI;
use sbi_spec::binary::SbiRet;
use spin::{Mutex, Once};
use trap_vec::{load_trap_vec, trap_vec};
use uart_16550::MmioSerialPort;

/// 栈空间。
#[link_section = ".bss.uninit"]
static mut ROOT_STACK: [Stack; NUM_HART_MAX] = [Stack::ZERO; NUM_HART_MAX];

/// 入口。
///
/// # Safety
///
/// 裸函数。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    asm!(
        "   la   sp, {stack}
            li   t0, {per_hart_stack_size}
            csrr t1,  mhartid
            addi t1,  t1,  1
         1: add  sp,  sp, t0
            addi t1,  t1, -1
            bnez t1,  1b
            call {move_stack}
            call {rust_main}
            j    {trap}
        ",
        per_hart_stack_size = const LEN_STACK_PER_HART,
        stack               =   sym ROOT_STACK,
        move_stack          =   sym fast_trap::reuse_stack_for_trap,
        rust_main           =   sym rust_main,
        trap                =   sym trap_vec,
        options(noreturn)
    )
}

#[naked]
unsafe extern "C" fn _stop() -> ! {
    asm!("wfi", options(noreturn))
}

/// rust 入口。
extern "C" fn rust_main(_hartid: usize, opaque: usize) {
    static GENESIS: AtomicBool = AtomicBool::new(true);
    static DONE: AtomicBool = AtomicBool::new(true);
    static BOARD_INFO: Once<BoardInfo> = Once::new();

    // 全局初始化过程
    if GENESIS.swap(false, Ordering::AcqRel) {
        extern "C" {
            static mut sbss: u64;
            static mut ebss: u64;
        }
        unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
        // 解析设备树
        let board_info = BOARD_INFO.call_once(|| device_tree::parse(opaque));
        // 初始化外设
        *UART.lock() = MaybeUninit::new(unsafe { MmioSerialPort::new(board_info.uart.start) });
        rcore_console::init_console(&Console);
        rcore_console::set_log_level(option_env!("LOG"));
        clint::init(board_info.clint.start);
        qemu_test::init(board_info.test.start);
        // 打印启动信息
        print!(
            "\
[rustsbi] RustSBI version {ver_sbi}, adapting to RISC-V SBI v1.0.0
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
            hartid = hart_id(),
            dtb = board_info.dtb,
            firmware = _start as usize,
        );
        // 初始化 SBI
        unsafe {
            SBI = MaybeUninit::new(
                rustsbi::Builder::new_machine()
                    .with_ipi(&clint::Clint)
                    .with_timer(&clint::Clint)
                    .with_hsm(Hsm)
                    .with_reset(qemu_test::get())
                    .build(),
            );
        };
        // 设置并打印 pmp
        set_pmp(board_info);
        hart_csr_utils::print_pmps();
        // 设置陷入栈
        unsafe { ROOT_STACK[hart_id()].load_as_stack() };
        // 设置内核入口
        unsafe {
            ROOT_STACK[hart_id()]
                .hart_context()
                .hsm
                .remote()
                .start(Supervisor {
                    start_addr: SUPERVISOR_ENTRY,
                    opaque,
                });
        }
        DONE.store(false, Ordering::SeqCst);
    } else {
        // 设置 pmp
        set_pmp(BOARD_INFO.wait());
        // 设置陷入栈
        unsafe { ROOT_STACK[hart_id()].load_as_stack() };
        while DONE.load(Ordering::SeqCst) {
            core::hint::spin_loop();
        }
    }
    // 清理 clint
    clint::msip::clear();
    clint::mtimecmp::clear();
    // 准备启动调度
    unsafe {
        asm!("csrw mcause, {}", in(reg) cause::BOOT);
        asm!("csrw mideleg, {}", in(reg) !0);
        asm!("csrw medeleg, {}", in(reg) !0);
        asm!("csrw mcounteren, {}", in(reg) !0);
        riscv::register::medeleg::clear_supervisor_env_call();
        riscv::register::medeleg::clear_machine_env_call();
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

mod cause {
    pub(crate) const BOOT: usize = 24;
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
        mcause::{self, Exception as E, Interrupt as I, Trap as T},
        mepc, mstatus, mtval,
    };

    let cause = mcause::read();
    // 启动
    if (cause.cause() == T::Exception(E::Unknown) && cause.bits() == cause::BOOT)
        || cause.cause() == T::Interrupt(I::MachineSoft)
    {
        let hart_id = hart_id();
        let hart_ctx = unsafe { ROOT_STACK[hart_id].hart_context() };
        match unsafe { hart_ctx.hsm.local() }.start() {
            Ok(supervisor) => {
                unsafe {
                    load_trap_vec(true);
                    mstatus::set_mpie();
                    mstatus::set_mpp(mstatus::MPP::Supervisor);
                    mie::write(mie::MSIE | mie::MTIE | mie::MEIE);
                }
                hart_ctx.trap.a[0] = hart_id;
                hart_ctx.trap.a[1] = supervisor.opaque;
                hart_ctx.trap.pc = supervisor.start_addr;
            }
            Err(_state) => {
                unsafe {
                    load_trap_vec(false);
                    mstatus::set_mpie();
                    mstatus::set_mpp(mstatus::MPP::Machine);
                    mie::write(mie::MSIE);
                };
                hart_ctx.trap.pc = _stop as usize;
            }
        }
        return ctx.call(2);
    }
    match cause.cause() {
        // SBI call
        T::Exception(E::SupervisorEnvCall) => {
            use sbi_spec::hsm;
            let ret = unsafe { SBI.assume_init_mut() }.handle_ecall(
                a7,
                a6,
                [ctx.a0(), a1, a2, a3, a4, a5],
            );
            if ret.is_ok() && a7 == hsm::EID_HSM {
                if a6 == hsm::HART_STOP {
                    unsafe {
                        load_trap_vec(true);
                        mstatus::set_mpp(mstatus::MPP::Machine);
                        mie::write(mie::MSIE);
                        ROOT_STACK[hart_id()].hart_context().trap.pc = _stop as usize;
                    }
                    return ctx.call(0);
                }
                if a6 == hsm::HART_SUSPEND
                    && ctx.a0() == hsm::HART_SUSPEND_TYPE_NON_RETENTIVE as usize
                {
                    unsafe {
                        load_trap_vec(false);
                        mstatus::set_mpp(mstatus::MPP::Machine);
                        ROOT_STACK[hart_id()].hart_context().trap.pc = _stop as usize;
                    }
                    return ctx.call(0);
                }
            }
            mepc::write(mepc::read() + 4);
            ctx.save_args(ret.value, a2, a3, a4, a5, a6, a7);
            ctx.write_a(0, ret.error);
            ctx.restore()
        }
        // 其他陷入
        trap => {
            let mstatus: usize;
            unsafe { asm!("csrr {}, mstatus", out(reg) mstatus) };
            println!(
                "
-----------------------------
> trap:    {trap:?}
> mstatus: {mstatus:#018x}
> mepc:    {:#018x}
> mtval:   {:#018x}
-----------------------------
            ",
                mepc::read(),
                mtval::read()
            );
            panic!("stopped with unsupported trap")
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

/// 类型化栈。
///
/// 每个硬件线程拥有一个满足这样条件的内存块。
/// 这个内存块的底部放着硬件线程状态 [`HartContext`]，顶部用于陷入处理，中间是这个硬件线程的栈空间。
/// 不需要 M 态线程，每个硬件线程只有这一个栈。
#[repr(C, align(128))]
struct Stack([u8; LEN_STACK_PER_HART]);

impl Stack {
    /// 零初始化以避免加载。
    const ZERO: Self = Self([0; LEN_STACK_PER_HART]);

    /// 从栈上取出硬件线程状态。
    #[inline]
    fn hart_context(&mut self) -> &mut HartContext {
        unsafe { &mut *self.0.as_mut_ptr().cast() }
    }

    fn load_as_stack(&'static mut self) {
        let hart = self.hart_context();
        hart.hsm = HsmCell::new();
        let ptr = unsafe { NonNull::new_unchecked(&mut hart.trap) };
        forget(
            FreeTrapStack::new(StackRef(self), ptr, fast_handler)
                .unwrap()
                .load(),
        );
    }
}

#[repr(transparent)]
struct StackRef(&'static mut Stack);

impl AsRef<[u8]> for StackRef {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0 .0[size_of::<HartContext>()..]
    }
}

impl AsMut<[u8]> for StackRef {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0 .0[size_of::<HartContext>()..]
    }
}

impl TrapStackBlock for StackRef {}

impl Drop for StackRef {
    fn drop(&mut self) {
        panic!("Root stack cannot be dropped")
    }
}

/// 硬件线程上下文。
#[repr(C)]
struct HartContext {
    /// 陷入上下文。
    trap: FlowContext,
    hsm: HsmCell<Supervisor>,
}

/// 特权软件信息。
#[derive(Debug)]
struct Supervisor {
    start_addr: usize,
    opaque: usize,
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

static mut SBI: MaybeUninit<FixedRustSBI> = MaybeUninit::uninit();

type FixedRustSBI<'a> = RustSBI<
    &'a clint::Clint,
    &'a clint::Clint,
    Infallible,
    Hsm,
    &'a qemu_test::QemuTest,
    Infallible,
>;

struct Hsm;

impl rustsbi::Hsm for Hsm {
    fn hart_start(&self, hartid: usize, start_addr: usize, opaque: usize) -> SbiRet {
        match unsafe { ROOT_STACK.get_mut(hartid).map(|s| s.hart_context()) } {
            Some(hart) => {
                if hart.hsm.remote().start(Supervisor { start_addr, opaque }) {
                    clint::msip::send(hartid);
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
        unsafe { ROOT_STACK[hart_id()].hart_context().hsm.local().stop() };
        SbiRet::success(0)
    }

    #[inline]
    fn hart_get_status(&self, hartid: usize) -> SbiRet {
        match unsafe { ROOT_STACK.get_mut(hartid).map(|s| s.hart_context()) } {
            Some(hart) => SbiRet::success(hart.hsm.remote().sbi_get_status()),
            None => SbiRet::invalid_param(),
        }
    }

    fn hart_suspend(&self, suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
        use sbi_spec::hsm as spec;
        match suspend_type {
            spec::HART_SUSPEND_TYPE_NON_RETENTIVE => unsafe {
                ROOT_STACK[hart_id()]
                    .hart_context()
                    .hsm
                    .local()
                    .suspend_non_retentive(Supervisor {
                        start_addr: resume_addr,
                        opaque,
                    });
                SbiRet::success(0)
            },
            spec::HART_SUSPEND_TYPE_RETENTIVE => unsafe {
                ROOT_STACK[hart_id()].hart_context().hsm.local().suspend();
                asm!(
                    "   la    {0}, 1f
                        csrrw {0}, mtvec,   {0}
                        csrrw {1}, mepc,    {1}
                        csrrw {2}, mstatus, {2}
                        wfi
                    1:  csrrw {2}, mstatus, {2}
                        csrrw {1}, mepc,    {1}
                        csrrw {0}, mtvec,   {0}
                    ",
                    out(reg) _,
                    out(reg) _,
                    out(reg) _,
                );
                ROOT_STACK[hart_id()].hart_context().hsm.local().resume();
                SbiRet::success(0)
            },
            _ => SbiRet::not_supported(),
        }
    }
}
