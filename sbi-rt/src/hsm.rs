//! Chapter 9. Hart State Management Extension (EID #0x48534D "HSM")

use crate::binary::{eid_from_str, sbi_call_0, sbi_call_1, sbi_call_3, SbiRet};
use fid::*;

pub const EID_HSM: usize = eid_from_str("HSM") as _;

pub const HART_STATE_STARTED: usize = 0;
pub const HART_STATE_STOPPED: usize = 1;
pub const HART_STATE_START_PENDING: usize = 2;
pub const HART_STATE_STOP_PENDING: usize = 3;
pub const HART_STATE_SUSPENDED: usize = 4;
pub const HART_STATE_SUSPEND_PENDING: usize = 5;
pub const HART_STATE_RESUME_PENDING: usize = 6;

pub const HART_SUSPEND_TYPE_RETENTIVE: u32 = 0;
pub const HART_SUSPEND_TYPE_NON_RETENTIVE: u32 = 0x8000_0000;

/// §9.1
#[inline]
pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(EID_HSM, HART_START, hartid, start_addr, opaque)
}

/// §9.2
#[inline]
pub fn hart_stop() -> SbiRet {
    sbi_call_0(EID_HSM, HART_STOP)
}

/// §9.3
#[inline]
pub fn hart_get_status(hartid: usize) -> SbiRet {
    sbi_call_1(EID_HSM, HART_GET_STATUS, hartid)
}

/// §9.4
#[inline]
pub fn hart_suspend(suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(
        EID_HSM,
        HART_SUSPEND,
        suspend_type as _,
        resume_addr,
        opaque,
    )
}

/// §9.5
mod fid {
    pub(super) const HART_START: usize = 0;
    pub(super) const HART_STOP: usize = 1;
    pub(super) const HART_GET_STATUS: usize = 2;
    pub(super) const HART_SUSPEND: usize = 3;
}
