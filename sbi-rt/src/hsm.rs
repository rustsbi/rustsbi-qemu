//! Chapter 9. Hart State Management Extension (EID #0x48534D "HSM")

use crate::binary::{eid_from_str, sbi_call_0, sbi_call_1, sbi_call_3, SbiRet};

pub const EID_HSM: usize = eid_from_str("HSM") as _;

const FID_HART_START: usize = 0;
const FID_HART_STOP: usize = 1;
const FID_HART_GET_STATUS: usize = 2;
const FID_HART_SUSPEND: usize = 3;

pub const HART_STATE_STARTED: usize = 0;
pub const HART_STATE_STOPPED: usize = 1;
pub const HART_STATE_START_PENDING: usize = 2;
pub const HART_STATE_STOP_PENDING: usize = 3;
pub const HART_STATE_SUSPENDED: usize = 4;
pub const HART_STATE_SUSPEND_PENDING: usize = 5;
pub const HART_STATE_RESUME_PENDING: usize = 6;

#[inline]
pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(EID_HSM, FID_HART_START, hartid, start_addr, opaque)
}

#[inline]
pub fn hart_stop() -> SbiRet {
    sbi_call_0(EID_HSM, FID_HART_STOP)
}

#[inline]
pub fn hart_get_status(hartid: usize) -> SbiRet {
    sbi_call_1(EID_HSM, FID_HART_GET_STATUS, hartid)
}

#[inline]
pub fn hart_suspend(suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(
        EID_HSM,
        FID_HART_SUSPEND,
        suspend_type as usize,
        resume_addr,
        opaque,
    )
}
