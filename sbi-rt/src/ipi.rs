//! Chapter 7. IPI Extension (EID #0x735049 "sPI: s-mode IPI")

#![deny(warnings)]

use crate::binary::{eid_from_str, sbi_call_2, SbiRet};

pub const EID_IPI: usize = eid_from_str("sPI") as _;

const FID_SEND_IPI: usize = 0;

#[inline]
pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> SbiRet {
    sbi_call_2(EID_IPI, FID_SEND_IPI, hart_mask, hart_mask_base)
}
