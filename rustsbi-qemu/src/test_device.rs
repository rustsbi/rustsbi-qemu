// SiFive Test virtual device
//
// This is a test finisher memory mapped device used to exit simulation
//
// Ref: https://github.com/qemu/qemu/blob/master/hw/misc/sifive_test.c
use rustsbi::{
    reset::{
        RESET_REASON_NO_REASON, RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_COLD_REBOOT,
        RESET_TYPE_SHUTDOWN, RESET_TYPE_WARM_REBOOT,
    },
    Reset, SbiRet,
};

// Zero sized structure for a static write-only device
pub struct SiFiveTest;

// Write these values to perform test device operations
const TEST_FAIL: u32 = 0x3333;
const TEST_PASS: u32 = 0x5555;
const TEST_RESET: u32 = 0x7777;

// On most QEMU host platforms, exit code for a general error is 1
const QEMU_ERR_EXIT_CODE: u32 = 1;

impl Reset for SiFiveTest {
    fn system_reset(&self, reset_type: usize, reset_reason: usize) -> SbiRet {
        const VIRT_TEST: *mut u32 = 0x10_0000 as *mut u32;
        let value = match reset_type {
            RESET_TYPE_SHUTDOWN => match reset_reason {
                RESET_REASON_NO_REASON => TEST_PASS,
                RESET_REASON_SYSTEM_FAILURE => TEST_FAIL | (QEMU_ERR_EXIT_CODE << 16),
                // pass unknown reason from [2, 0xFFFF] to qemu return value output
                // reason if reason <= 0xFFFF => TEST_FAIL | (((reason & 0xFFFF) as u32) << 16),
                _ => return SbiRet::invalid_param(),
            },
            RESET_TYPE_COLD_REBOOT => TEST_RESET,
            RESET_TYPE_WARM_REBOOT => TEST_RESET,
            _ => return SbiRet::invalid_param(),
        };
        unsafe {
            core::ptr::write_volatile(VIRT_TEST, value);
        }
        unreachable!()
    }
}
