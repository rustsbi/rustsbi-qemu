//! Chapter 7. IPI Extension (EID #0x735049 "sPI: s-mode IPI")

use crate::binary::{eid_from_str, sbi_call_2, SbiRet};
use fid::*;

pub const EID_SPI: usize = eid_from_str("sPI") as _;

/// §7.1
#[inline]
pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> SbiRet {
    sbi_call_2(EID_SPI, SEND_IPI, hart_mask, hart_mask_base)
}

/// §7.2
mod fid {
    pub(super) const SEND_IPI: usize = 0;
}
