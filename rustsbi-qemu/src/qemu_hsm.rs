//! Hart state monitor designed for QEMU

use crate::{clint::Clint, entry, hart_id, NUM_HART_MAX, SUPERVISOR_ENTRY};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicU8, Ordering},
};
use rustsbi::SbiRet;
use spin::Mutex;

pub(crate) const SUSPEND_RETENTIVE: u32 = 0x00000000;
pub(crate) const SUSPEND_NON_RETENTIVE: u32 = 0x80000000;
pub(crate) const EID_HSM: usize = 0x48534D;
pub(crate) const FID_HART_STOP: usize = 1;
pub(crate) const FID_HART_SUSPEND: usize = 3;

const STARTED: u8 = 0;
const STOPPED: u8 = 1;
const START_PENDING: u8 = 2;
const STOP_PENDING: u8 = 3;
const SUSPEND: u8 = 4;
const SUSPEND_PENDING: u8 = 5;
const RESUME_PENDING: u8 = 6;

pub(crate) struct QemuHsm {
    clint: &'static Clint,
    state: [AtomicU8; NUM_HART_MAX],
    supervisor: [Mutex<Option<Supervisor>>; NUM_HART_MAX],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Supervisor {
    pub start_addr: usize,
    pub opaque: usize,
}

impl QemuHsm {
    pub fn new(clint: &'static Clint, smp: usize, opaque: usize) -> Self {
        let state: MaybeUninit<[AtomicU8; NUM_HART_MAX]> = MaybeUninit::uninit();
        let supervisor: MaybeUninit<[Mutex<Option<Supervisor>>; NUM_HART_MAX]> =
            MaybeUninit::uninit();

        let mut state = unsafe { state.assume_init() };
        let mut supervisor = unsafe { supervisor.assume_init() };
        for id in 0..smp {
            if id == hart_id() {
                state[id] = AtomicU8::new(START_PENDING);
                supervisor[id] = Mutex::new(Some(Supervisor {
                    start_addr: SUPERVISOR_ENTRY,
                    opaque,
                }));
            } else {
                state[id] = AtomicU8::new(STOP_PENDING);
                supervisor[id] = Mutex::new(None);
            }
        }

        Self {
            clint,
            state,
            supervisor,
        }
    }

    /// 读取操作系统入口地址准备跳转。
    pub fn take_supervisor(&self) -> Option<Supervisor> {
        self.supervisor[hart_id()].lock().take()
    }

    /// 为硬件线程准备休眠或关闭。
    ///
    /// 此时核状态必然是不可干预的 Pending 状态，中断业已关闭。
    pub fn record_ready_to_reboot(&self) {
        use core::sync::atomic::Ordering::{AcqRel, Acquire};
        use riscv::{
            interrupt,
            register::{mie, mip, mtvec},
        };

        // 检查当前状态是重启前的休眠状态
        let state = &self.state[hart_id()];
        let current = state.load(Acquire);
        let new: u8 = match current {
            STOP_PENDING => STOPPED,
            SUSPEND_PENDING => SUSPEND,
            s => panic!("wrong state {s:?}!"),
        };

        // TODO: SBI 在 M 态应该总是处于这样干净的状态，即：
        // 1. 所有中断标记清除
        // 2. 所有中断已关闭
        //
        // 这样，发生状态转换时只需要：
        // 1. 重设 mtvec
        // 2. 开启需要的中断
        self.clint.clear_soft(hart_id());
        unsafe {
            mip::clear_msoft();
            mie::set_msoft();
            mtvec::write(entry as _, mtvec::TrapMode::Direct);
        }
        if let Err(unexpected) = state.compare_exchange(current, new, AcqRel, Acquire) {
            panic!("failed to reboot for a race {current:?} => {unexpected:?}");
        }
        unsafe { interrupt::enable() };
    }

    /// Record that current hart id is marked as `Started` state.
    /// It is used when hart stop command is received in interrupt handler.
    /// The target hart (when in interrupt handler) is prepared to start, it marks itself into 'started',
    /// and should jump to target address right away.
    pub fn record_current_start_finished(&self) {
        self.state[hart_id()].store(STARTED, Ordering::Release);
    }
}

// Adapt RustSBI interface to RustSBI-QEMU's QemuHsm.
impl rustsbi::Hsm for &'static QemuHsm {
    fn hart_start(&self, hart_id: usize, start_addr: usize, opaque: usize) -> SbiRet {
        use riscv::register::mstatus::{self, MPP};

        // previous privileged mode should be user or supervisor; start from machine mode is not supported
        if !matches!(mstatus::read().mpp(), MPP::Supervisor | MPP::User) {
            return SbiRet::invalid_param();
        }
        // try to modify state to start hart
        let state = if let Some(s) = self.state.get(hart_id) {
            s
        } else {
            return SbiRet::invalid_param();
        };

        match state.compare_exchange(
            STOPPED as _,
            START_PENDING as _,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(STARTED) => return SbiRet::already_available(),
            Err(_) => return SbiRet::failed(),
        }
        // todo: check start address
        // SBI_ERR_INVALID_ADDRESS: start_addr is not valid possibly due to following reasons:
        // - It is not a valid physical address.
        // - The address is prohibited by PMP to run in supervisor mode. */
        *self.supervisor[hart_id].lock() = Some(Supervisor { start_addr, opaque });
        loop {
            self.clint.clear_soft(hart_id);
            self.clint.send_soft(hart_id);
            for _ in 0..0x20000 {
                unsafe { riscv::asm::nop() };
            }
            if state.load(Ordering::Acquire) != START_PENDING as _ {
                break;
            }
        }
        // this does not block the current function
        // The following process is going to be handled in software interrupt handler,
        // and the function returns immediately as starting a hart is defined as an asynchronous procedure.
        SbiRet::ok(0)
    }

    fn hart_stop(&self) -> SbiRet {
        match self.state[hart_id()].compare_exchange(
            STARTED,
            STOP_PENDING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                *self.supervisor[hart_id()].lock() = None;
                SbiRet::ok(0)
            }
            Err(_) => SbiRet::failed(),
        }
    }

    fn hart_get_status(&self, hart_id: usize) -> SbiRet {
        self.state.get(hart_id).map_or(
            SbiRet::invalid_param(), // not in `state` map structure, the given hart id is invalid
            |s| SbiRet::ok(s.load(Ordering::Acquire) as _),
        )
    }

    fn hart_suspend(&self, suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
        match self.state[hart_id()].compare_exchange(
            STARTED,
            SUSPEND_PENDING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => match suspend_type {
                SUSPEND_RETENTIVE => todo!(),
                SUSPEND_NON_RETENTIVE => {
                    *self.supervisor[hart_id()].lock() = Some(Supervisor {
                        start_addr: resume_addr,
                        opaque,
                    });
                    SbiRet::ok(0)
                }
                _ => SbiRet::not_supported(),
            },
            Err(_) => SbiRet::failed(),
        }
    }
}
