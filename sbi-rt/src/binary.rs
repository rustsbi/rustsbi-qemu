//! Capture 3. Binary Encoding

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
pub const SBI_ERR_FAILED: usize = -1isize as _;
pub const SBI_ERR_NOT_SUPPORTED: usize = -2isize as _;
pub const SBI_ERR_INVALID_PARAM: usize = -3isize as _;
pub const SBI_ERR_DENIED: usize = -4isize as _;
pub const SBI_ERR_INVALID_ADDRESS: usize = -5isize as _;
pub const SBI_ERR_ALREADY_AVAILABLE: usize = -6isize as _;
pub const SBI_ERR_ALREADY_STARTED: usize = -7isize as _;
pub const SBI_ERR_ALREADY_STOPPED: usize = -8isize as _;

impl core::fmt::Debug for SbiRet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.error {
            SBI_SUCCESS => self.value.fmt(f),
            SBI_ERR_FAILED => write!(f, "<SBI call failed>"),
            SBI_ERR_NOT_SUPPORTED => write!(f, "<SBI feature not supported>"),
            SBI_ERR_INVALID_PARAM => write!(f, "<SBI invalid parameter>"),
            SBI_ERR_DENIED => write!(f, "<SBI denied>"),
            SBI_ERR_INVALID_ADDRESS => write!(f, "<SBI invalid address>"),
            SBI_ERR_ALREADY_AVAILABLE => write!(f, "<SBI already available>"),
            SBI_ERR_ALREADY_STARTED => write!(f, "<SBI already started>"),
            SBI_ERR_ALREADY_STOPPED => write!(f, "<SBI already stopped>"),
            unknown => write!(f, "[SBI Unknown error: {unknown:#x}]"),
        }
    }
}

pub enum Error {
    Failed,
    NotSupported,
    InvalidParam,
    Denied,
    InvalidAddress,
    AlreadyAvailable,
    AlreadyStarted,
    AlreadyStopped,
    Customed(isize),
}

impl SbiRet {
    pub const fn result(&self) -> Result<usize, Error> {
        match self.error {
            SBI_SUCCESS => Ok(self.value),
            SBI_ERR_FAILED => Err(Error::Failed),
            SBI_ERR_NOT_SUPPORTED => Err(Error::NotSupported),
            SBI_ERR_INVALID_PARAM => Err(Error::InvalidParam),
            SBI_ERR_DENIED => Err(Error::Denied),
            SBI_ERR_INVALID_ADDRESS => Err(Error::InvalidAddress),
            SBI_ERR_ALREADY_AVAILABLE => Err(Error::AlreadyAvailable),
            SBI_ERR_ALREADY_STARTED => Err(Error::AlreadyStarted),
            SBI_ERR_ALREADY_STOPPED => Err(Error::AlreadyStopped),
            unknown => Err(Error::Customed(unknown as _)),
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

#[cfg(target_pointer_width = "32")]
#[inline(always)]
pub(crate) fn sbi_call_6(
    eid: usize,
    fid: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
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
            in("a5") arg5,
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
