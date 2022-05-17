//! Chapter 9. Hart State Management Extension (EID #0x48534D "HSM")

#![deny(warnings)]

use crate::binary::{eid_from_str, sbi_call_0, sbi_call_1, sbi_call_3, SbiRet};

pub const EID_HSM: usize = eid_from_str("HSM") as _;

const FID_HART_START: usize = 0;
const FID_HART_STOP: usize = 1;
const FID_HART_GET_STATUS: usize = 2;
const FID_HART_SUSPEND: usize = 3;

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
