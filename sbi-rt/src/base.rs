//! Chapter 4. Base Extension (EID #0x10)

use crate::binary::{sbi_call_0, sbi_call_1};
use fid::*;

pub const EID_BASE: usize = 0x10;
pub use impl_id::*;

/// §4.1
#[inline]
pub fn get_spec_version() -> usize {
    sbi_call_0(EID_BASE, GET_SPEC_VERSION).value
}

/// §4.2
#[inline]
pub fn get_sbi_impl_id() -> usize {
    sbi_call_0(EID_BASE, GET_SBI_IMPL_ID).value
}

/// §4.3
#[inline]
pub fn get_sbi_impl_version() -> usize {
    sbi_call_0(EID_BASE, GET_SBI_IMPL_VERSION).value
}

/// §4.4
#[inline]
pub fn probe_extension(extension_id: usize) -> usize {
    sbi_call_1(EID_BASE, PROBE_EXTENSION, extension_id).value
}

/// §4.5
#[inline]
pub fn get_mvendorid() -> usize {
    sbi_call_0(EID_BASE, GET_MVENDORID).value
}

/// §4.6
#[inline]
pub fn get_marchid() -> usize {
    sbi_call_0(EID_BASE, GET_MARCHID).value
}

/// §4.7
#[inline]
pub fn get_mimpid() -> usize {
    sbi_call_0(EID_BASE, GET_MIMPID).value
}

/// §4.8
mod fid {
    pub(super) const GET_SPEC_VERSION: usize = 0x0;
    pub(super) const GET_SBI_IMPL_ID: usize = 0x1;
    pub(super) const GET_SBI_IMPL_VERSION: usize = 0x2;
    pub(super) const PROBE_EXTENSION: usize = 0x3;
    pub(super) const GET_MVENDORID: usize = 0x4;
    pub(super) const GET_MARCHID: usize = 0x5;
    pub(super) const GET_MIMPID: usize = 0x6;
}

/// §4.9
mod impl_id {
    pub const IMPL_BBL: usize = 0;
    pub const IMPL_OPEN_SBI: usize = 1;
    pub const IMPL_XVISOR: usize = 2;
    pub const IMPL_KVM: usize = 3;
    pub const IMPL_RUST_SBI: usize = 4;
    pub const IMPL_DIOSIX: usize = 5;
    pub const IMPL_COFFER: usize = 6;
}
