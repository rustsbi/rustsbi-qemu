//! Capture 3. Binary Encoding

#![deny(warnings)]

/// SBI functions return type.
///
/// > SBI functions must return a pair of values in a0 and a1,
/// > with a0 returning an error code.
/// > This is analogous to returning the C structure `SbiRet`.
#[repr(C)]
pub struct SbiRet {
    /// Error number
    pub error: usize,
    /// Result value
    pub value: usize,
}

pub const SBI_SUCCESS: usize = 0;
pub const SBI_ERR_FAILED: usize = error_code(-1);
pub const SBI_ERR_NOT_SUPPORTED: usize = error_code(-2);
pub const SBI_ERR_INVALID_PARAM: usize = error_code(-3);
pub const SBI_ERR_DENIED: usize = error_code(-4);
pub const SBI_ERR_INVALID_ADDRESS: usize = error_code(-5);
pub const SBI_ERR_ALREADY_AVAILABLE: usize = error_code(-6);
pub const SBI_ERR_ALREADY_STARTED: usize = error_code(-7);
pub const SBI_ERR_ALREADY_STOPPED: usize = error_code(-8);

impl core::fmt::Debug for SbiRet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use core::write;
        match self.error {
            SBI_SUCCESS => write!(f, "{:?}", self.value),
            SBI_ERR_FAILED => write!(f, "<SBI call failed>"),
            SBI_ERR_NOT_SUPPORTED => write!(f, "<SBI feature not supported>"),
            SBI_ERR_INVALID_PARAM => write!(f, "<SBI invalid parameter>"),
            SBI_ERR_DENIED => write!(f, "<SBI denied>"),
            SBI_ERR_INVALID_ADDRESS => write!(f, "<SBI invalid address>"),
            SBI_ERR_ALREADY_AVAILABLE => write!(f, "<SBI already available>"),
            SBI_ERR_ALREADY_STARTED => write!(f, "<SBI already started>"),
            SBI_ERR_ALREADY_STOPPED => write!(f, "<SBI already stopped>"),
            unknown => write!(f, "[SBI Unknown error: {unknown}]"),
        }
    }
}

#[inline(always)]
pub(crate) fn sbi_call_0(eid: usize, fid: usize) -> SbiRet {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            lateout("a0") error,
            lateout("a1") value,
        );
    }
    SbiRet { error, value }
}

#[inline(always)]
pub(crate) fn sbi_call_1(eid: usize, fid: usize, arg0: usize) -> SbiRet {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            in("a0") arg0,
            lateout("a0") error,
            lateout("a1") value,
        );
    }
    SbiRet { error, value }
}

#[inline(always)]
pub(crate) fn sbi_call_2(eid: usize, fid: usize, arg0: usize, arg1: usize) -> SbiRet {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            in("a0") arg0,
            in("a1") arg1,
            lateout("a0") error,
            lateout("a1") value,
        );
    }
    SbiRet { error, value }
}

#[inline(always)]
pub(crate) fn sbi_call_3(eid: usize, fid: usize, arg0: usize, arg1: usize, arg2: usize) -> SbiRet {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            lateout("a0") error,
            lateout("a1") value,
        );
    }
    SbiRet { error, value }
}

#[inline(always)]
pub(crate) fn sbi_call_4(
    eid: usize,
    fid: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> SbiRet {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            in("a3") arg3,
            lateout("a0") error,
            lateout("a1") value,
        );
    }
    SbiRet { error, value }
}

#[inline(always)]
pub(crate) fn sbi_call_5(
    eid: usize,
    fid: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
) -> SbiRet {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            in("a3") arg3,
            in("a4") arg4,
            lateout("a0") error,
            lateout("a1") value,
        );
    }
    SbiRet { error, value }
}

/// Converts SBI EID from str.
pub(crate) const fn eid_from_str(name: &str) -> i32 {
    match *name.as_bytes() {
        [a] => a as _,
        [a, b] => (a as i32) << 8 | b as i32,
        [a, b, c] => (a as i32) << 16 | (b as i32) << 8 | c as i32,
        [a, b, c, d] => (a as i32) << 24 | (b as i32) << 16 | (c as i32) << 8 | d as i32,
        _ => unreachable!(),
    }
}

const fn error_code(val: i32) -> usize {
    usize::from_ne_bytes(isize::to_ne_bytes(val as _))
}
