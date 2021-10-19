//! Hart state monitor designed for QEMU

use hashbrown::HashMap;
use rustsbi::SbiRet;
use core::sync::atomic::{AtomicU8, Ordering};
use alloc::sync::Arc;
use riscv::register::mstatus::{self, MPP};

// RISC-V SBI Hart State Monitor states
#[allow(unused)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
// and last command (see HsmCommand) when hart is requested to procceed HSM functions. 
//
// RustSBI-QEMU makes use of machine software interrupt. Functions should modify `state` to
// XxxPending before the actual procedure began. Then, caller should store next command structure
// to `last_command`, and use IPI to invoke software interrupt on machine level.
// 
// When target hart received machine software interrupt, it should read and procceed command
// from `last_command`. Then, after command execution makes progress, it should modify
// `state` to Xxxed to mark that the HSM function has taken effect.
//
// These functions above are defined as asynchronous procedures. That means it returns before
// acutal procedure has finished. There are functions to read its current state when the target hart 
// is still in transition or after the transition is done. These functions may read from `last_command`
// variable at any time.
#[derive(Clone)]
pub struct QemuHsm {
    state: Arc<spin::Mutex<HashMap<usize, AtomicU8>>>,
    last_command: Arc<spin::Mutex<HashMap<usize, HsmCommand>>>,
}

// RustSBI-QEMU HSM command, these commands apply to a given hart. 
// 
// Should be stored with hart id before software interrupt is invoked.
// After software interrupt is received, the target hart should handle with HSM command structure
// and run corresponding HSM procedures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HsmCommand {
    Start(usize, usize),
    Stop,
}

impl QemuHsm {
    // creates a RustSBI-QEMU hsm structure.
    pub fn new() -> Self {
        Self {
            state: Arc::new(spin::Mutex::new(HashMap::new())),
            last_command: Arc::new(spin::Mutex::new(HashMap::new())),
        }
    }
    // Return last command by current hart id.
    // This function is used in software interrupt handler to check which HSM function should we execute.
    pub(crate) fn last_command(&self) -> Option<HsmCommand> {
        let hart_id = riscv::register::mhartid::read();
        let last_command_lock = self.last_command.lock();
        let ans = last_command_lock.get(&hart_id).map(|c| *c);
        drop(last_command_lock);
        ans
    }
    // Record that current hart id is marked as `Stopped` state.
    // It is used in interrupt handler, when hart stop command is received. Before this function,
    // the target hart is making preparations to stop; it records state and must stop immediately after 
    // this function is called.
    pub(crate) fn record_current_stop_finished(&self) {
        let hart_id = riscv::register::mhartid::read();
        self.state.lock().entry(hart_id)
            .insert(AtomicU8::new(HsmState::Stopped as u8));
    }
    // Record that current hart id is marked as `Started` state.
    // It is used when hart stop command is received in interrupt handler.
    // The target hart (when in interrupt handler) is prepared to start, it marks itself into 'started', 
    // and should jump to target address right away.
    pub(crate) fn record_current_start_finished(&self) {
        let hart_id = riscv::register::mhartid::read();
        self.state.lock().entry(hart_id)
            .insert(AtomicU8::new(HsmState::Started as u8));
    }
}

// Adapt RustSBI interface to RustSBI-QEMU's QemuHsm.
impl rustsbi::Hsm for QemuHsm {
    // The supervisor software above RustSBI has called SBI environment to start a given `hart_id` 
    // to address `start_addr` with parameter `opaque`.
    fn hart_start(&self, hart_id: usize, start_addr: usize, opaque: usize) -> SbiRet {
        // previous privileged mode should be user or supervisor; start from machine mode is not supported
        let mpp = mstatus::read().mpp();
        if mpp != MPP::Supervisor && mpp != MPP::User {
            return SbiRet::invalid_param()
        }          
        // try to modify state to start hart
        let mut state_lock = self.state.lock();
        let current_state = state_lock.entry(hart_id)
            .or_insert(AtomicU8::new(HsmState::Stopped as u8))
            .compare_exchange(
                HsmState::Stopped as u8,
                HsmState::StartPending as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            ); 
        // procceed with invalid hart states.
        // - the given hartid is already started, the compare exchange should fail and suggests current state as `Started`,
        // function should return error as already available.
        if current_state == Err(HsmState::Started as u8) {
            return SbiRet::already_available()
        } 
        // - otherwise return invalid parameter, this may be caused for hart is already transitioning from started state
        if current_state != Ok(HsmState::Stopped as u8) {
            return SbiRet::invalid_param()
        }
        // todo: check start address
        /* SBI_ERR_INVALID_ADDRESS: start_addr is not valid possibly due to following reasons:
        * It is not a valid physical address.
        * The address is prohibited by PMP to run in supervisor mode. */
        // fill in the parameter
        let mut config_lock = self.last_command.lock();
        config_lock.entry(hart_id).insert(HsmCommand::Start(start_addr, opaque));
        drop(config_lock);
        drop(state_lock);
        // now, start the target hart
        let clint = crate::clint::Clint::new(0x2000000 as *mut u8);
        clint.send_soft(hart_id); // this does not block the current function
        // The following process is going to be handled in software interrupt handler, and
        // the function returns immediately as starting a hart is defined as an asynchronous procedure.
        SbiRet::ok(0)
    }
    fn hart_stop(&self, hart_id: usize) -> SbiRet {
        // try to set current target hart state to stop pending
        let mut state_lock = self.state.lock();
        let current_state = state_lock.entry(hart_id)
            .or_insert(AtomicU8::new(HsmState::Stopped as u8))
            .compare_exchange(
                HsmState::Started as u8,
                HsmState::StopPending as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            ); 
        // check current hart state
        if current_state == Err(HsmState::Started as u8) {
            return SbiRet::failed() // illegal state
        }
        // fill in the parameter
        let mut config_lock = self.last_command.lock();
        config_lock.entry(hart_id).insert(HsmCommand::Stop);
        drop(config_lock);
        drop(state_lock); 
        // stop the target hart
        let clint = crate::clint::Clint::new(0x2000000 as *mut u8);
        clint.send_soft(hart_id);
        SbiRet::ok(0)
    }
    fn hart_get_status(&self, hart_id: usize) -> SbiRet {
        self.state.lock().get(&hart_id).map_or(
            SbiRet::invalid_param(), // not in `state` map structure, the given hart id is invalid
            |a| SbiRet::ok(a.load(Ordering::Relaxed) as usize)
        )
    }
    // Supervisor requested current hart to suspend.
    //
    // In RustSBI-QEMU, if `suspend_type` is retentive, it pauses the current hart; `resume_addr` 
    // and `opaque` is not used.
    // Otherwise, the current hart discards current supervisor context, and returns to another
    //  `resume_addr` with parameter `opaque`.
    fn hart_suspend(&self, suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
        match suspend_type {    
            // Resuming from a retentive suspend state is straight forward and the supervisor-mode software 
            // will see SBI suspend call return without any failures.
            SUSPEND_RETENTIVE => {
                pause(); // pause and wait for machine level ipi
                SbiRet::ok(0)
            },
            // Resuming from a non-retentive suspend state is relatively more involved and requires software 
            // to restore various hart registers and CSRs for all privilege modes. 
            // Upon resuming from non-retentive suspend state, the hart will jump to supervisor-mode at address 
            // specified by `resume_addr` with specific registers values described in the table below:
            //
            // | Register Name | Register Value
            // |:--------------|:--------------
            // | `satp`        | 0
            // | `sstatus.SIE` | 0
            // | a0            | hartid
            // | a1            | `opaque` parameter
            SUSPEND_NON_RETENTIVE => {
                pause();
                unsafe {
                    riscv::register::satp::write(0);
                    riscv::register::sstatus::clear_sie();
                }
                match () {
                    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
                    () => unsafe {
                        asm!(
                            "csrr   a0, mhartid",
                            "jr     {resume_addr}",
                            resume_addr = in(reg) resume_addr,
                            in("a1") opaque,
                            options(noreturn)
                        )
                    },
                    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
                    () => {
                        drop((resume_addr, opaque));
                        unimplemented!("not RISC-V instruction set architecture")
                    }
                };
            },
            // There could be other platform specific suspend types; RustSBI-QEMU does not define any
            // platform suspend types. It gives SBI return value as not supported.
            _ => SbiRet::not_supported()
        }
    }
}

const SUSPEND_RETENTIVE: u32 = 0x00000000;
const SUSPEND_NON_RETENTIVE: u32 = 0x80000000;

// Pause current hart, wake through inter-processor interrupt
pub fn pause() {
    use riscv::asm::wfi;
    use riscv::register::{mie, mip, mhartid};
    use crate::clint::Clint;
    unsafe { 
        let hartid = mhartid::read();
        let clint = Clint::new(0x2000000 as *mut u8);
        clint.clear_soft(hartid); // Clear IPI
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