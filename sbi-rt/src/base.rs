//! Chapter 4. Base Extension (EID #0x10)

use crate::binary::{sbi_call_0, sbi_call_1};

pub const EID_BASE: usize = 0x10;

const FID_GET_SPEC_VERSION: usize = 0x0;
const FID_GET_SBI_IMPL_ID: usize = 0x1;
const FID_GET_SBI_IMPL_VERSION: usize = 0x2;
const FID_PROBE_EXTENSION: usize = 0x3;
const FID_GET_MVENDORID: usize = 0x4;
const FID_GET_MARCHID: usize = 0x5;
const FID_GET_MIMPID: usize = 0x6;

#[inline]
pub fn get_spec_version() -> usize {
    sbi_call_0(EID_BASE, FID_GET_SPEC_VERSION).value
}

#[inline]
pub fn get_sbi_impl_id() -> usize {
    sbi_call_0(EID_BASE, FID_GET_SBI_IMPL_ID).value
}

#[inline]
pub fn get_sbi_impl_version() -> usize {
    sbi_call_0(EID_BASE, FID_GET_SBI_IMPL_VERSION).value
}

#[inline]
pub fn probe_extension(extension_id: usize) -> usize {
    sbi_call_1(EID_BASE, FID_PROBE_EXTENSION, extension_id).value
}

#[inline]
pub fn get_mvendorid() -> usize {
    sbi_call_0(EID_BASE, FID_GET_MVENDORID).value
}

#[inline]
pub fn get_marchid() -> usize {
    sbi_call_0(EID_BASE, FID_GET_MARCHID).value
}

#[inline]
pub fn get_mimpid() -> usize {
    sbi_call_0(EID_BASE, FID_GET_MIMPID).value
}
