//! Hart state monitor designed for QEMU

use crate::{clint::Clint, entry, hart_id, set_mtcev, Supervisor, NUM_HART_MAX, SUPERVISOR_ENTRY};
use core::{mem::MaybeUninit, sync::atomic::AtomicU8};
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

impl QemuHsm {
    pub fn new(clint: &'static Clint, smp: usize, opaque: usize) -> Self {
        let state: MaybeUninit<[AtomicU8; NUM_HART_MAX]> = MaybeUninit::uninit();
        let supervisor: MaybeUninit<[Mutex<Option<Supervisor>>; NUM_HART_MAX]> =
            MaybeUninit::uninit();

        let mut state = unsafe { state.assume_init() };
        let mut supervisor = unsafe { supervisor.assume_init() };
        for id in 0..smp {
            state[id] = AtomicU8::new(START_PENDING);
            supervisor[id] = Mutex::new(
                // 执行全局初始化的硬件线程将直通特权软件
                if id == hart_id() {
                    Some(Supervisor {
                        start_addr: SUPERVISOR_ENTRY,
                        opaque,
                    })
                }
                // 否则将在下一个步骤被关闭
                else {
                    None
                },
            );
        }

        Self {
            clint,
            state,
            supervisor,
        }
    }

    /// 读取特权态入口地址，转换状态准备跳转。
    pub fn take_supervisor(&self) -> Option<Supervisor> {
        use core::sync::atomic::Ordering::{AcqRel, Acquire};

        // 检查当前状态是启动前的挂起状态
        let state = &self.state[hart_id()];
        let supervisor = self.supervisor[hart_id()].lock().take();

        let current = state.load(Acquire);
        let new: u8 = match current {
            START_PENDING => {
                if supervisor.is_none() {
                    // 在启动过程中但未设置特权态入口，转入关闭流程
                    STOP_PENDING
                } else {
                    // 在启动过程中且已设置特权态入口，继续启动
                    return supervisor;
                }
            }
            SUSPEND => {
                if supervisor.is_none() {
                    // 在挂起状态但未设置特权态入口，无法恢复
                    panic!("cannot resume without supervisor!")
                } else {
                    // 在挂起状态且已设置特权态入口，转入恢复流程
                    RESUME_PENDING
                }
            }
            s => panic!("wrong state {s:?}!"),
        };

        match state.compare_exchange(current, new, AcqRel, Acquire) {
            Ok(_) => supervisor,
            Err(unexpected) => panic!("failed to reboot for a race {current:?} => {unexpected:?}"),
        }
    }

    /// 为硬件线程准备休眠或关闭。
    ///
    /// 此时核状态必然是不可干预的 Pending 状态，中断业已关闭。
    pub fn finallize_before_stop(&self) {
        use core::sync::atomic::Ordering::{AcqRel, Acquire};
        use riscv::register::mie;

        // 检查当前状态是重启前的挂起状态
        let state = &self.state[hart_id()];
        let current = state.load(Acquire);
        let new: u8 = match current {
            STOP_PENDING => {
                // 一旦关闭，只能通过软件中断重启
                unsafe { mie::clear_mext() };
                STOPPED
            }
            SUSPEND_PENDING => {
                // 休眠也可以通过外部中断唤醒
                unsafe { mie::set_mext() };
                SUSPEND
            }
            s => panic!("wrong state {s:?}!"),
        };
        // 通过软件中断重启
        unsafe {
            mie::set_msoft();
            set_mtcev(entry as _)
        };
        // 转移状态
        if let Err(unexpected) = state.compare_exchange(current, new, AcqRel, Acquire) {
            panic!("failed to reboot for a race {current:?} => {unexpected:?}")
        }
    }

    /// Record that current hart id is marked as `Started` state.
    /// It is used when hart stop command is received in interrupt handler.
    /// The target hart (when in interrupt handler) is prepared to start, it marks itself into 'started',
    /// and should jump to target address right away.
    pub fn record_current_start_finished(&self) {
        use core::sync::atomic::Ordering::Release;
        self.state[hart_id()].store(STARTED, Release);
    }

    /// 如果一个核可以接受 ipi，返回 `true`。
    ///
    /// 运行状态的核可以接受权限低于 SBI 软件的核间中断，将转交给特权软件。
    /// 挂起状态的核可以接受核间中断以恢复运行。
    pub fn is_ipi_allowed(&self, hart_id: usize) -> bool {
        use core::sync::atomic::Ordering::Acquire;
        self.state
            .get(hart_id)
            .map_or(false, |s| matches!(s.load(Acquire), STARTED | SUSPEND))
    }
}

// Adapt RustSBI interface to RustSBI-QEMU's QemuHsm.
impl rustsbi::Hsm for &'static QemuHsm {
    fn hart_start(&self, hart_id: usize, start_addr: usize, opaque: usize) -> SbiRet {
        use core::sync::atomic::Ordering::{AcqRel, Acquire};
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

        match state.compare_exchange(STOPPED, START_PENDING, AcqRel, Acquire) {
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
                core::hint::spin_loop();
            }
            if state.load(Acquire) != START_PENDING {
                break;
            }
        }
        // this does not block the current function
        // The following process is going to be handled in software interrupt handler,
        // and the function returns immediately as starting a hart is defined as an asynchronous procedure.
        SbiRet::ok(0)
    }

    fn hart_stop(&self) -> SbiRet {
        use core::sync::atomic::Ordering::{AcqRel, Acquire};
        match self.state[hart_id()].compare_exchange(STARTED, STOP_PENDING, AcqRel, Acquire) {
            Ok(_) => {
                *self.supervisor[hart_id()].lock() = None;
                SbiRet::ok(0)
            }
            Err(_) => SbiRet::failed(),
        }
    }

    fn hart_get_status(&self, hart_id: usize) -> SbiRet {
        use core::sync::atomic::Ordering::Acquire;
        self.state.get(hart_id).map_or(
            SbiRet::invalid_param(), // not in `state` map structure, the given hart id is invalid
            |s| SbiRet::ok(s.load(Acquire) as _),
        )
    }

    fn hart_suspend(&self, suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
        use core::sync::atomic::Ordering::{AcqRel, Acquire};
        match self.state[hart_id()].compare_exchange(STARTED, SUSPEND_PENDING, AcqRel, Acquire) {
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
