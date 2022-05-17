//! Chapter 6. Timer Extension (EID #0x54494D45 "TIME")

#![deny(warnings)]

use crate::binary::{eid_from_str, sbi_call_1, SbiRet};

pub const EID_TIMER: usize = eid_from_str("TIME") as _;

const FID_SET_TIMER: usize = 0;

#[inline]
pub fn set_timer(stime_value: u64) -> SbiRet {
    match () {
        #[cfg(target_pointer_width = "32")]
        () => sbi_call_2(
            EID_TIMER,
            FID_SET_TIMER,
            stime_value as _,
            (stime_value >> 32) as _,
        ),
        #[cfg(target_pointer_width = "64")]
        () => sbi_call_1(EID_TIMER, FID_SET_TIMER, stime_value as _),
    }
}
