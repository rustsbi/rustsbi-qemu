//! Chapter 8. RFENCE Extension (EID #0x52464E43 "RFNC")

#![deny(warnings)]

use crate::binary::{eid_from_str, sbi_call_2, sbi_call_4, sbi_call_5, SbiRet};

pub const EID_RFENCE: usize = eid_from_str("RFNC") as _;

const FID_REMOTE_FENCE_I: usize = 0;
const FID_REMOTE_SFENCE_VMA: usize = 1;
const FID_REMOTE_SFENCE_VMA_ASID: usize = 2;
const FID_REMOTE_HFENCE_GVMA_VMID: usize = 3;
const FID_REMOTE_HFENCE_GVMA: usize = 4;
const FID_REMOTE_HFENCE_VVMA_ASID: usize = 5;
const FID_REMOTE_HFENCE_VVMA: usize = 6;

#[inline]
pub fn remote_fence_i(hart_mask: usize, hart_mask_base: usize) -> SbiRet {
    sbi_call_2(EID_RFENCE, FID_REMOTE_FENCE_I, hart_mask, hart_mask_base)
}

#[inline]
pub fn remote_sfence_vma(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
) -> SbiRet {
    sbi_call_4(
        EID_RFENCE,
        FID_REMOTE_SFENCE_VMA,
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
    )
}

#[inline]
pub fn remote_sfence_vma_asid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    asid: usize,
) -> SbiRet {
    sbi_call_5(
        EID_RFENCE,
        FID_REMOTE_SFENCE_VMA_ASID,
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
        asid,
    )
}

#[inline]
pub fn remote_hfence_gvma_vmid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    vmid: usize,
) -> SbiRet {
    sbi_call_5(
        EID_RFENCE,
        FID_REMOTE_HFENCE_GVMA_VMID,
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
        vmid,
    )
}

#[inline]
pub fn remote_hfence_gvma(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
) -> SbiRet {
    sbi_call_4(
        EID_RFENCE,
        FID_REMOTE_HFENCE_GVMA_VMID,
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
    )
}

#[inline]
pub fn remote_hfence_vvma_asid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    asid: usize,
) -> SbiRet {
    sbi_call_5(
        EID_RFENCE,
        FID_REMOTE_HFENCE_GVMA_VMID,
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
        asid,
    )
}

#[inline]
pub fn remote_hfence_vvma(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
) -> SbiRet {
    sbi_call_4(
        EID_RFENCE,
        FID_REMOTE_HFENCE_GVMA_VMID,
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
    )
}
