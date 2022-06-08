//! Chapter 10. System Reset Extension (EID #0x53525354 "SRST")

use crate::binary::{eid_from_str, sbi_call_2, SbiRet};
use fid::*;

pub const EID_SRST: usize = eid_from_str("SRST") as _;

pub const RESET_TYPE_SHUTDOWN: u32 = 0;
pub const RESET_TYPE_COLD_REBOOT: u32 = 1;
pub const RESET_TYPE_WARM_REBOOT: u32 = 2;

pub const RESET_REASON_NO_REASON: u32 = 0;
pub const RESET_REASON_SYSTEM_FAILURE: u32 = 1;

/// §10.1
#[inline]
pub fn system_reset(reset_type: u32, reset_reason: u32) -> SbiRet {
    sbi_call_2(EID_SRST, SYSTEM_RESET, reset_type as _, reset_reason as _)
}

/// §10.2
mod fid {
    pub(super) const SYSTEM_RESET: usize = 0;
}
