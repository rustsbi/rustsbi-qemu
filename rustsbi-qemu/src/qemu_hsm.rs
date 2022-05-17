//! Hart state monitor designed for QEMU

use alloc::sync::Arc;
use hashbrown::HashMap;
use riscv::register::mstatus::{self, MPP};
use rustsbi::SbiRet;

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

// RustSBI-QEMU hart state monitor structure. It stores hart states for all harts,
// and last command (see HsmCommand) when hart is requested to proceed HSM functions.
//
// RustSBI-QEMU makes use of machine software interrupt. Functions should modify `state` to
// XxxPending before the actual procedure began. Then, caller should store next command structure
// to `last_command`, and use IPI to invoke software interrupt on machine level.
//
// When target hart received machine software interrupt, it should read and proceed command
// from `last_command`. Then, after command execution makes progress, it should modify
// `state` variable to mark that the HSM function has taken effect.
//
// These functions above are defined as asynchronous procedures. That means it returns before
// actual procedure has finished. There are functions to read its current state when the target hart
// is still in transition or after the transition is done. These functions may read from `last_command`
// variable at any time.
#[derive(Clone, Default)]
pub struct QemuHsm {
    state: Arc<spin::Mutex<HashMap<usize, HsmState>>>,
    last_command: Arc<spin::Mutex<HashMap<usize, HsmCommand>>>,
}

// RustSBI-QEMU HSM command, these commands apply to a remote given hart.
//
// Should be stored with hart id before software interrupt is invoked.
// After software interrupt is received, the target hart should handle with HSM command structure
// and run corresponding HSM procedures.
//
// By current version of SBI specification, suspend command only apply to current hart,
// thus RustSBI does not use remote HSM command in this case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HsmCommand {
    Start(usize, usize),
    Stop,
}

impl QemuHsm {
    // Return last command by current hart id.
    // This function is used in software interrupt handler to check which HSM function should we execute.
    pub(crate) fn last_command(&self) -> Option<HsmCommand> {
        let hart_id = riscv::register::mhartid::read();
        self.last_command.lock().get(&hart_id).copied()
    }
    // Record that current hart id is marked as `Stopped` state.
    // It is used in interrupt handler, when hart stop command is received. Before this function,
    // the target hart is making preparations to stop; it records state and must stop immediately after
    // this function is called.
    pub(crate) fn record_current_stop_finished(&self) {
        let hart_id = riscv::register::mhartid::read();
        self.state.lock().entry(hart_id).insert(HsmState::Stopped);
    }
    // Record that current hart id is marked as `Started` state.
    // It is used when hart stop command is received in interrupt handler.
    // The target hart (when in interrupt handler) is prepared to start, it marks itself into 'started',
    // and should jump to target address right away.
    pub(crate) fn record_current_start_finished(&self) {
        let hart_id = riscv::register::mhartid::read();
        self.state.lock().entry(hart_id).insert(HsmState::Started);
    }
}

// Adapt RustSBI interface to RustSBI-QEMU's QemuHsm.
impl rustsbi::Hsm for QemuHsm {
    fn hart_start(&self, hart_id: usize, start_addr: usize, opaque: usize) -> SbiRet {
        // previous privileged mode should be user or supervisor; start from machine mode is not supported
        if !matches!(mstatus::read().mpp(), MPP::Supervisor | MPP::User) {
            return SbiRet::invalid_param();
        }
        // try to modify state to start hart
        match self.state.lock().get_mut(&hart_id) {
            Some(s) if *s == HsmState::Stopped => *s = HsmState::StartPending,
            Some(s) if *s == HsmState::Started => return SbiRet::already_available(),
            Some(_) | None => return SbiRet::invalid_param(),
        }
        // todo: check start address
        // SBI_ERR_INVALID_ADDRESS: start_addr is not valid possibly due to following reasons:
        // - It is not a valid physical address.
        // - The address is prohibited by PMP to run in supervisor mode. */
        self.last_command
            .lock()
            .insert(hart_id, HsmCommand::Start(start_addr, opaque));
        crate::clint::get().send_soft(hart_id);
        // this does not block the current function
        // The following process is going to be handled in software interrupt handler,
        // and the function returns immediately as starting a hart is defined as an asynchronous procedure.
        SbiRet::ok(0)
    }

    fn hart_stop(&self) -> SbiRet {
        let hart_id = riscv::register::mhartid::read();
        // try to modify state to stop hart
        match self.state.lock().get_mut(&hart_id) {
            Some(s) if *s == HsmState::Started => *s = HsmState::StopPending,
            Some(_) | None => return SbiRet::invalid_param(),
        }
        self.last_command.lock().insert(hart_id, HsmCommand::Stop);
        crate::clint::get().send_soft(hart_id);
        SbiRet::ok(0)
    }

    fn hart_get_status(&self, hart_id: usize) -> SbiRet {
        self.state.lock().get(&hart_id).map_or(
            SbiRet::invalid_param(), // not in `state` map structure, the given hart id is invalid
            |s| SbiRet::ok(*s as usize),
        )
    }

    fn hart_suspend(&self, suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
        let hart_id = riscv::register::mhartid::read();
        // try to modify state to suspend hart
        match self.state.lock().get_mut(&hart_id) {
            Some(s) if *s == HsmState::Started => *s = HsmState::SuspendPending,
            Some(_) => return SbiRet::failed(),
            None => return SbiRet::invalid_param(),
        }
        // pause and wait for machine level ipi
        suspend_current_hart(self);
        match suspend_type {
            // Resuming from a retentive suspend state is straight forward
            // and the supervisor-mode software will see SBI suspend call return without any failures.
            SUSPEND_RETENTIVE => {
                // mark current hart as started
                self.state.lock().insert(hart_id, HsmState::Started);
                SbiRet::ok(0)
            }
            // Resuming from a non-retentive suspend state is relatively more involved
            // and requires software to restore various hart registers and CSRs for all privilege modes.
            SUSPEND_NON_RETENTIVE => {
                // send start command to runtime of current hart
                self.last_command
                    .lock()
                    .insert(hart_id, HsmCommand::Start(resume_addr, opaque));
                crate::clint::get().send_soft(hart_id);
                unreachable!()
            }
            // There could be other platform specific suspend types; RustSBI-QEMU does not define any
            // platform suspend types. It gives SBI return value as not supported.
            _ => SbiRet::not_supported(),
        }
    }
}

const SUSPEND_RETENTIVE: u32 = 0x00000000;
const SUSPEND_NON_RETENTIVE: u32 = 0x80000000;

// Suspend current hart and record resume state when wake
pub fn suspend_current_hart(hsm: &QemuHsm) {
    use riscv::asm::wfi;
    use riscv::register::{mhartid, mie, mip};
    let hart_id = mhartid::read();
    let clint = crate::clint::get();
    clint.clear_soft(hart_id); // Clear IPI
    unsafe { mip::clear_msoft() }; // clear machine software interrupt flag
    let prev_msoft = mie::read().msoft();
    unsafe { mie::set_msoft() }; // Start listening for software interrupts
                                 // mark current state as suspended
    hsm.state.lock().entry(hart_id).insert(HsmState::Suspended);
    // actual suspended process
    loop {
        unsafe { wfi() };
        if mip::read().msoft() {
            break;
        }
    }
    // mark current state as resume pending
    hsm.state
        .lock()
        .entry(hart_id)
        .insert(HsmState::ResumePending);
    // resume
    if !prev_msoft {
        unsafe { mie::clear_msoft() }; // Stop listening for software interrupts
    }
    clint.clear_soft(hart_id); // Clear IPI
}

// Pause current hart, wake through inter-processor interrupt
pub fn pause() {
    use riscv::asm::wfi;
    use riscv::register::{mhartid, mie, mip};
    unsafe {
        let hartid = mhartid::read();
        let clint = crate::clint::get();
        clint.clear_soft(hartid); // Clear IPI
        mip::clear_msoft(); // clear machine software interrupt flag
        let prev_msoft = mie::read().msoft();
        mie::set_msoft(); // Start listening for software interrupts
        loop {
            wfi();
            if mip::read().msoft() {
                break;
            }
        }
        if !prev_msoft {
            mie::clear_msoft(); // Stop listening for software interrupts
        }
        clint.clear_soft(hartid); // Clear IPI
    }
}
