#![allow(unused)]
use core::arch::asm;
use core::fmt;

pub const EXTENSION_BASE: usize = 0x10;
pub const EXTENSION_TIMER: usize = 0x54494D45;
pub const EXTENSION_IPI: usize = 0x735049;
pub const EXTENSION_RFENCE: usize = 0x52464E43;
pub const EXTENSION_HSM: usize = 0x48534D;
pub const EXTENSION_SRST: usize = 0x53525354;

const FUNCTION_BASE_GET_SPEC_VERSION: usize = 0x0;
const FUNCTION_BASE_GET_SBI_IMPL_ID: usize = 0x1;
const FUNCTION_BASE_GET_SBI_IMPL_VERSION: usize = 0x2;
const FUNCTION_BASE_PROBE_EXTENSION: usize = 0x3;
const FUNCTION_BASE_GET_MVENDORID: usize = 0x4;
const FUNCTION_BASE_GET_MARCHID: usize = 0x5;
const FUNCTION_BASE_GET_MIMPID: usize = 0x6;

#[repr(C)]
pub struct SbiRet {
    /// Error number
    pub error: usize,
    /// Result value
    pub value: usize,
}

const SBI_SUCCESS: usize = 0;
const SBI_ERR_FAILED: usize = usize::from_ne_bytes(isize::to_ne_bytes(-1));
const SBI_ERR_NOT_SUPPORTED: usize = usize::from_ne_bytes(isize::to_ne_bytes(-2));
const SBI_ERR_INVALID_PARAM: usize = usize::from_ne_bytes(isize::to_ne_bytes(-3));
const SBI_ERR_DENIED: usize = usize::from_ne_bytes(isize::to_ne_bytes(-4));
const SBI_ERR_INVALID_ADDRESS: usize = usize::from_ne_bytes(isize::to_ne_bytes(-5));
const SBI_ERR_ALREADY_AVAILABLE: usize = usize::from_ne_bytes(isize::to_ne_bytes(-6));
const SBI_ERR_ALREADY_STARTED: usize = usize::from_ne_bytes(isize::to_ne_bytes(-7));
const SBI_ERR_ALREADY_STOPPED: usize = usize::from_ne_bytes(isize::to_ne_bytes(-8));

impl fmt::Debug for SbiRet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            unknown => write!(f, "[SBI Unknown error: {}]", unknown),
        }
    }
}

#[inline]
pub fn get_spec_version() -> usize {
    sbi_call_0(EXTENSION_BASE, FUNCTION_BASE_GET_SPEC_VERSION).value
}

#[inline]
pub fn get_sbi_impl_id() -> usize {
    sbi_call_0(EXTENSION_BASE, FUNCTION_BASE_GET_SBI_IMPL_ID).value
}

#[inline]
pub fn get_sbi_impl_version() -> usize {
    sbi_call_0(EXTENSION_BASE, FUNCTION_BASE_GET_SBI_IMPL_VERSION).value
}

#[inline]
pub fn probe_extension(extension_id: usize) -> usize {
    sbi_call_1(EXTENSION_BASE, FUNCTION_BASE_PROBE_EXTENSION, extension_id).value
}

#[inline]
pub fn get_mvendorid() -> usize {
    sbi_call_0(EXTENSION_BASE, FUNCTION_BASE_GET_MVENDORID).value
}

#[inline]
pub fn get_marchid() -> usize {
    sbi_call_0(EXTENSION_BASE, FUNCTION_BASE_GET_MARCHID).value
}

#[inline]
pub fn get_mimpid() -> usize {
    sbi_call_0(EXTENSION_BASE, FUNCTION_BASE_GET_MIMPID).value
}

const FUNCTION_SYSTEM_RESET: usize = 0x0;

pub const RESET_TYPE_SHUTDOWN: usize = 0x0000_0000;
pub const RESET_TYPE_COLD_REBOOT: usize = 0x0000_0001;
pub const RESET_TYPE_WARM_REBOOT: usize = 0x0000_0002;
pub const RESET_REASON_NO_REASON: usize = 0x0000_0000;
pub const RESET_REASON_SYSTEM_FAILURE: usize = 0x0000_0001;

#[inline]
pub fn reset(reset_type: usize, reset_reason: usize) -> SbiRet {
    sbi_call_2(
        EXTENSION_SRST,
        FUNCTION_SYSTEM_RESET,
        reset_type,
        reset_reason,
    )
}

pub fn shutdown() -> ! {
    sbi_call_2(
        EXTENSION_SRST,
        FUNCTION_SYSTEM_RESET,
        RESET_TYPE_SHUTDOWN,
        RESET_REASON_NO_REASON,
    );
    unreachable!()
}

#[inline(always)]
fn sbi_call_legacy(which: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let ret;
    match () {
        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        () => unsafe {
            asm!(
                "ecall",
                in("a0") arg0, in("a1") arg1, in("a2") arg2,
                in("a7") which,
                lateout("a0") ret,
            )
        },
        #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
        () => {
            drop((which, arg0, arg1, arg2));
            unimplemented!("not RISC-V instruction set architecture")
        }
    };
    ret
}

const SBI_SET_TIMER: usize = 0;
const SBI_CONSOLE_PUTCHAR: usize = 1;
const SBI_CONSOLE_GETCHAR: usize = 2;
const SBI_CLEAR_IPI: usize = 3;
const SBI_SEND_IPI: usize = 4;
const SBI_REMOTE_FENCE_I: usize = 5;
const SBI_REMOTE_SFENCE_VMA: usize = 6;
const SBI_REMOTE_SFENCE_VMA_ASID: usize = 7;
const SBI_SHUTDOWN: usize = 8;

pub fn console_putchar(c: usize) {
    sbi_call_legacy(SBI_CONSOLE_PUTCHAR, c, 0, 0);
}

pub fn console_getchar() -> usize {
    sbi_call_legacy(SBI_CONSOLE_GETCHAR, 0, 0, 0)
}

pub fn set_timer(time: usize) {
    sbi_call_legacy(SBI_SET_TIMER, time, 0, 0);
}

const FUNCTION_IPI_SEND_IPI: usize = 0x0;

pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> SbiRet {
    sbi_call_2(
        EXTENSION_IPI,
        FUNCTION_IPI_SEND_IPI,
        hart_mask,
        hart_mask_base,
    )
}

const FUNCTION_HSM_HART_START: usize = 0x0;
const FUNCTION_HSM_HART_STOP: usize = 0x1;
const FUNCTION_HSM_HART_GET_STATUS: usize = 0x2;
const FUNCTION_HSM_HART_SUSPEND: usize = 0x3;

pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(
        EXTENSION_HSM,
        FUNCTION_HSM_HART_START,
        hartid,
        start_addr,
        opaque,
    )
}

pub fn hart_stop(hartid: usize) -> SbiRet {
    sbi_call_1(EXTENSION_HSM, FUNCTION_HSM_HART_STOP, hartid)
}

pub fn hart_get_status(hartid: usize) -> SbiRet {
    sbi_call_1(EXTENSION_HSM, FUNCTION_HSM_HART_GET_STATUS, hartid)
}

pub fn hart_suspend(suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiRet {
    sbi_call_3(
        EXTENSION_HSM,
        FUNCTION_HSM_HART_SUSPEND,
        suspend_type as usize,
        resume_addr,
        opaque,
    )
}

#[inline(always)]
fn sbi_call_0(extension: usize, function: usize) -> SbiRet {
    let (error, value);
    match () {
        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        () => unsafe {
            asm!(
                "ecall",
                in("a6") function, in("a7") extension,
                lateout("a0") error, lateout("a1") value,
            )
        },
        #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
        () => {
            drop((extension, function));
            unimplemented!("not RISC-V instruction set architecture")
        }
    };
    SbiRet { error, value }
}

#[inline(always)]
fn sbi_call_1(extension: usize, function: usize, arg0: usize) -> SbiRet {
    let (error, value);
    match () {
        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        () => unsafe {
            asm!(
                "ecall",
                in("a0") arg0,
                in("a6") function, in("a7") extension,
                lateout("a0") error, lateout("a1") value,
            )
        },
        #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
        () => {
            drop((extension, function, arg0));
            unimplemented!("not RISC-V instruction set architecture")
        }
    };
    SbiRet { error, value }
}

#[inline(always)]
fn sbi_call_2(extension: usize, function: usize, arg0: usize, arg1: usize) -> SbiRet {
    let (error, value);
    match () {
        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        () => unsafe {
            asm!(
                "ecall",
                in("a0") arg0, in("a1") arg1,
                in("a6") function, in("a7") extension,
                lateout("a0") error, lateout("a1") value,
            )
        },
        #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
        () => {
            drop((extension, function, arg0, arg1));
            unimplemented!("not RISC-V instruction set architecture")
        }
    };
    SbiRet { error, value }
}

#[inline(always)]
fn sbi_call_3(extension: usize, function: usize, arg0: usize, arg1: usize, arg2: usize) -> SbiRet {
    let (error, value);
    match () {
        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        () => unsafe {
            asm!(
                "ecall",
                in("a0") arg0, in("a1") arg1, in("a2") arg2,
                in("a6") function, in("a7") extension,
                lateout("a0") error, lateout("a1") value,
            )
        },
        #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
        () => {
            drop((extension, function, arg0, arg1, arg2));
            unimplemented!("not RISC-V instruction set architecture")
        }
    };
    SbiRet { error, value }
}
