//! Hart state monitor designed for QEMU

use crate::{clint::Clint, constants::SUPERVISOR_ENTRY, hart_id};
use alloc::{vec, vec::Vec};
use rustsbi::SbiRet;
use spin::Mutex;

pub(crate) const SUSPEND_RETENTIVE: u32 = 0x00000000;
pub(crate) const SUSPEND_NON_RETENTIVE: u32 = 0x80000000;
pub(crate) const EID_HSM: usize = 0x48534D;
pub(crate) const FID_HART_STOP: usize = 1;
pub(crate) const FID_HART_SUSPEND: usize = 3;

// RISC-V SBI Hart State Monitor states
#[allow(unused)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum HsmState {
    /// The hart is physically powered-up and executing normally.
    Started = 0,
    /// The hart is not executing in supervisor-mode or any lower privilege mode.
    /// It is probably powered-down by the SBI implementation if the underlying platform has a mechanism
    /// to physically power-down harts.
    Stopped = 1,
    /// Some other hart has requested to start (or power-up) the hart from the STOPPED state
    /// and the SBI implementation is still working to get the hart in the STARTED state.
    StartPending = 2,
    /// The hart has requested to stop (or power-down) itself from the STARTED state
    /// and the SBI implementation is still working to get the hart in the STOPPED state.
    StopPending = 3,
    /// This hart is in a platform specific suspend (or low power) state.
    Suspended = 4,
    /// The hart has requested to put itself in a platform specific low power state from the STARTED state
    /// and the SBI implementation is still working to get the hart in the platform specific SUSPENDED state.
    SuspendPending = 5,
    /// An interrupt or platform specific hardware event has caused the hart to resume normal execution from
    /// the SUSPENDED state and the SBI implementation is still working to get the hart in the STARTED state.
    ResumePending = 6,
}

/// RustSBI-QEMU hart state monitor structure.
///
/// It stores hart states for all harts,
/// and last command (see [`HsmCommand`]) when hart is requested to proceed HSM functions.
///
/// RustSBI-QEMU makes use of machine software interrupt.
/// Functions should modify `state` to XxxPending before the actual procedure began.
/// Then, caller should store next command structure to `last_command`,
/// and use IPI to invoke software interrupt on machine level.
///
/// When target hart received machine software interrupt,
/// it should read and proceed command from `last_command`.
/// Then, after command execution makes progress,
/// it should modify `state` variable to mark that the HSM function has taken effect.
///
/// These functions above are defined as asynchronous procedures.
/// That means it returns before actual procedure has finished.
/// There are functions to read its current state
/// when the target hart is still in transition or after the transition is done.
/// These functions may read from `last_command` variable at any time.
pub(crate) struct QemuHsm {
    clint: &'static Clint,
    state: Mutex<Vec<HsmState>>,
    supervisor: Mutex<Vec<Option<Supervisor>>>,
}

/// RustSBI-QEMU HSM command, these commands apply to a remote given hart.
///
/// Should be stored with hart id before software interrupt is invoked.
/// After software interrupt is received,
/// the target hart should handle with HSM command structure and run corresponding HSM procedures.
///
/// By current version of SBI specification, suspend command only apply to current hart,
/// thus RustSBI does not use remote HSM command in this case.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Supervisor {
    pub start_addr: usize,
    pub opaque: usize,
}

impl QemuHsm {
    pub fn new(clint: &'static Clint, smp: usize, opaque: usize) -> Self {
        let mut state = vec![HsmState::StopPending; smp];
        state[hart_id()] = HsmState::StartPending;

        let mut supervisor = vec![None; smp];
        supervisor[hart_id()] = Some(Supervisor {
            start_addr: SUPERVISOR_ENTRY,
            opaque,
        });

        Self {
            clint,
            state: Mutex::new(state),
            supervisor: Mutex::new(supervisor),
        }
    }

    /// Return last command by current hart id.
    /// This function is used in software interrupt handler to check which HSM function should we execute.
    pub fn take_supervisor(&self) -> Option<Supervisor> {
        self.supervisor.lock()[hart_id()].take()
    }

    /// Record that current hart id is marked as `Stopped` state.
    /// It is used in interrupt handler, when hart stop command is received. Before this function,
    /// the target hart is making preparations to stop;
    /// it records state and must stop immediately after this function is called.
    pub fn record_ready_to_reboot(&self) {
        match self.state.lock().get_mut(hart_id()) {
            Some(s) if *s == HsmState::StopPending => *s = HsmState::Stopped,
            Some(s) if *s == HsmState::SuspendPending => *s = HsmState::Suspended,
            Some(s) => panic!("wrong state {s:?}!"),
            None => unreachable!(),
        };
    }

    /// Record that current hart id is marked as `Started` state.
    /// It is used when hart stop command is received in interrupt handler.
    /// The target hart (when in interrupt handler) is prepared to start, it marks itself into 'started',
    /// and should jump to target address right away.
    pub fn record_current_start_finished(&self) {
        self.state.lock()[hart_id()] = HsmState::Started;
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
        {
            match self.state.lock().get_mut(hart_id) {
                Some(s) if *s == HsmState::Stopped => *s = HsmState::StartPending,
                Some(s) if *s == HsmState::Started => return SbiRet::already_available(),
                Some(_) => return SbiRet::failed(),
                None => return SbiRet::invalid_param(),
            };
        }
        // todo: check start address
        // SBI_ERR_INVALID_ADDRESS: start_addr is not valid possibly due to following reasons:
        // - It is not a valid physical address.
        // - The address is prohibited by PMP to run in supervisor mode. */
        self.supervisor.lock()[hart_id] = Some(Supervisor { start_addr, opaque });
        while self.state.lock()[hart_id] == HsmState::StartPending {
            self.clint.send_soft(hart_id);
            unsafe { riscv::asm::delay(0x20_0000) };
        }
        // this does not block the current function
        // The following process is going to be handled in software interrupt handler,
        // and the function returns immediately as starting a hart is defined as an asynchronous procedure.
        SbiRet::ok(0)
    }

    fn hart_stop(&self) -> SbiRet {
        // try to modify state to suspend hart
        {
            match self.state.lock().get_mut(hart_id()) {
                Some(s) if *s == HsmState::Started => *s = HsmState::StopPending,
                Some(_) | None => return SbiRet::failed(),
            };
        }
        self.supervisor.lock()[hart_id()] = None;
        SbiRet::ok(0)
    }

    fn hart_get_status(&self, hart_id: usize) -> SbiRet {
        self.state.lock().get(hart_id).map_or(
            SbiRet::invalid_param(), // not in `state` map structure, the given hart id is invalid
            |s| SbiRet::ok(*s as usize),
        )
    }

    fn hart_suspend(&self, suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
        {
            match self.state.lock().get_mut(hart_id()) {
                Some(s) if *s == HsmState::Started => *s = HsmState::SuspendPending,
                Some(_) | None => return SbiRet::failed(),
            };
        }
        match suspend_type {
            SUSPEND_RETENTIVE => todo!(),
            SUSPEND_NON_RETENTIVE => {
                self.supervisor.lock()[hart_id()] = Some(Supervisor {
                    start_addr: resume_addr,
                    opaque,
                });
                SbiRet::ok(0)
            }
            _ => SbiRet::not_supported(),
        }
    }
}
