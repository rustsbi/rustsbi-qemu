//! Chapter 5. Legacy Extensions (EIDs #0x00 - #0x0F)

const SBI_SET_TIMER: usize = 0;
const SBI_CONSOLE_PUTCHAR: usize = 1;
const SBI_CONSOLE_GETCHAR: usize = 2;
const SBI_CLEAR_IPI: usize = 3;
const SBI_SEND_IPI: usize = 4;
const SBI_REMOTE_FENCE_I: usize = 5;
const SBI_REMOTE_SFENCE_VMA: usize = 6;
const SBI_REMOTE_SFENCE_VMA_ASID: usize = 7;
const SBI_SHUTDOWN: usize = 8;

#[deprecated = "replaced by `set_timer` from Timer extension"]
#[inline]
pub fn set_timer(stime_value: u64) -> usize {
    match () {
        #[cfg(target_pointer_width = "32")]
        () => sbi_call_legacy_2(SBI_SET_TIMER, stime_value as _, (stime_value >> 32) as _),
        #[cfg(target_pointer_width = "64")]
        () => sbi_call_legacy_1(SBI_SET_TIMER, stime_value as _),
    }
}

#[deprecated = "no replacement"]
#[inline]
pub fn console_putchar(c: usize) -> usize {
    sbi_call_legacy_1(SBI_CONSOLE_PUTCHAR, c)
}

#[deprecated = "no replacement"]
#[inline]
pub fn console_getchar() -> usize {
    sbi_call_legacy_0(SBI_CONSOLE_GETCHAR)
}

#[deprecated = "you can clear `sip.SSIP` CSR bit directly"]
#[inline]
pub fn clear_ipi() -> usize {
    sbi_call_legacy_0(SBI_CLEAR_IPI)
}

#[deprecated = "replaced by `send_ipi` from IPI extension"]
#[inline]
pub fn send_ipi(hart_mask: usize) -> usize {
    sbi_call_legacy_1(SBI_SEND_IPI, hart_mask)
}

#[deprecated = "replaced by `remote_fence_i` from RFENCE extension"]
#[inline]
pub fn remote_fence_i(hart_mask: usize) -> usize {
    sbi_call_legacy_1(SBI_REMOTE_FENCE_I, hart_mask)
}

#[deprecated = "replaced by `remote_fence_vma` from RFENCE extension"]
#[inline]
pub fn remote_fence_vma(hart_mask: usize, start: usize, size: usize) -> usize {
    sbi_call_legacy_3(SBI_REMOTE_SFENCE_VMA, hart_mask, start, size)
}

#[deprecated = "replaced by `remote_fence_vma_asid` from RFENCE extension"]
#[inline]
pub fn remote_fence_vma_asid(hart_mask: usize, start: usize, size: usize, asid: usize) -> usize {
    sbi_call_legacy_4(SBI_REMOTE_SFENCE_VMA_ASID, hart_mask, start, size, asid)
}

#[deprecated = "replaced by `system_reset` from System Reset extension"]
#[inline]
pub fn shutdown() -> ! {
    sbi_call_legacy_0(SBI_SHUTDOWN);
    core::unreachable!()
}

#[inline(always)]
fn sbi_call_legacy_0(eid: usize) -> usize {
    let error;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            lateout("a0") error,
        );
    }
    error
}

#[inline(always)]
fn sbi_call_legacy_1(eid: usize, arg0: usize) -> usize {
    let error;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a0") arg0,
            lateout("a0") error,
        );
    }
    error
}

#[cfg(target_pointer_width = "32")]
#[inline(always)]
fn sbi_call_legacy_2(eid: usize, arg0: usize, arg1: usize) -> usize {
    let error;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a0") arg0,
            in("a1") arg1,
            lateout("a0") error,
        );
    }
    error
}

#[inline(always)]
fn sbi_call_legacy_3(eid: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let error;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            lateout("a0") error,
        );
    }
    error
}

#[inline(always)]
fn sbi_call_legacy_4(eid: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let error;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            in("a3") arg3,
            lateout("a0") error,
        );
    }
    error
}
