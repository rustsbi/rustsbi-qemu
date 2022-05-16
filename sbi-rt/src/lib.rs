#![no_std]
#![allow(unused)]

use core::{arch::asm, fmt, unreachable, write};

// §3
mod binary;
// §4
mod base;
// §5
pub mod legacy;

pub use base::*;
pub use binary::SbiRet;

use binary::{eid_from_str, sbi_call_0, sbi_call_1, sbi_call_2, sbi_call_3};

pub const EXTENSION_TIMER: usize = eid_from_str("TIME") as _;
pub const EXTENSION_IPI: usize = eid_from_str("sPI") as _;
pub const EXTENSION_RFENCE: usize = eid_from_str("RFNC") as _;
pub const EXTENSION_HSM: usize = eid_from_str("HSM") as _;
pub const EXTENSION_SRST: usize = eid_from_str("SRST") as _;

const FUNCTION_SYSTEM_RESET: usize = 0x0;

pub const RESET_TYPE_SHUTDOWN: usize = 0x0000_0000;
pub const RESET_TYPE_COLD_REBOOT: usize = 0x0000_0001;
pub const RESET_TYPE_WARM_REBOOT: usize = 0x0000_0002;
pub const RESET_REASON_NO_REASON: usize = 0x0000_0000;
pub const RESET_REASON_SYSTEM_FAILURE: usize = 0x0000_0001;

#[inline]
pub fn reset(reset_type: usize, reset_reason: usize) -> SbiRet {
    sbi_call_2(
        EXTENSION_SRST,
        FUNCTION_SYSTEM_RESET,
        reset_type,
        reset_reason,
    )
}

pub fn shutdown() -> ! {
    sbi_call_2(
        EXTENSION_SRST,
        FUNCTION_SYSTEM_RESET,
        RESET_TYPE_SHUTDOWN,
        RESET_REASON_NO_REASON,
    );
    unreachable!()
}

const FUNCTION_IPI_SEND_IPI: usize = 0x0;

pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> SbiRet {
    sbi_call_2(
        EXTENSION_IPI,
        FUNCTION_IPI_SEND_IPI,
        hart_mask,
        hart_mask_base,
    )
}

const FUNCTION_HSM_HART_START: usize = 0x0;
const FUNCTION_HSM_HART_STOP: usize = 0x1;
const FUNCTION_HSM_HART_GET_STATUS: usize = 0x2;
const FUNCTION_HSM_HART_SUSPEND: usize = 0x3;

pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(
        EXTENSION_HSM,
        FUNCTION_HSM_HART_START,
        hartid,
        start_addr,
        opaque,
    )
}

pub fn hart_stop(hartid: usize) -> SbiRet {
    sbi_call_1(EXTENSION_HSM, FUNCTION_HSM_HART_STOP, hartid)
}

pub fn hart_get_status(hartid: usize) -> SbiRet {
    sbi_call_1(EXTENSION_HSM, FUNCTION_HSM_HART_GET_STATUS, hartid)
}

pub fn hart_suspend(suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(
        EXTENSION_HSM,
        FUNCTION_HSM_HART_SUSPEND,
        suspend_type as usize,
        resume_addr,
        opaque,
    )
}
