//! Chapter 10. System Reset Extension (EID #0x53525354 "SRST")

#![deny(warnings)]

use crate::binary::{eid_from_str, sbi_call_2, SbiRet};

pub const EID_SYSTEM_RESET: usize = eid_from_str("SRST") as _;

const FID_SYSTEM_RESET: usize = 0;

pub const RESET_TYPE_SHUTDOWN: u32 = 0x0000_0000;
pub const RESET_TYPE_COLD_REBOOT: u32 = 0x0000_0001;
pub const RESET_TYPE_WARM_REBOOT: u32 = 0x0000_0002;

pub const RESET_REASON_NO_REASON: u32 = 0x0000_0000;
pub const RESET_REASON_SYSTEM_FAILURE: u32 = 0x0000_0001;

#[inline]
pub fn system_reset(reset_type: u32, reset_reason: u32) -> SbiRet {
    sbi_call_2(
        EID_SYSTEM_RESET,
        FID_SYSTEM_RESET,
        reset_type as _,
        reset_reason as _,
    )
}
