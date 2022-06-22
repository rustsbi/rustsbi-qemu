use qemu_exit::{QEMUExit, RISCV64};
use rustsbi::{
    spec::{
        binary::SbiRet,
        srst::{
            RESET_REASON_NO_REASON, RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_COLD_REBOOT,
            RESET_TYPE_SHUTDOWN, RESET_TYPE_WARM_REBOOT,
        },
    },
    Reset,
};
use spin::Once;

pub(crate) struct QemuTest(RISCV64);

static TEST: Once<QemuTest> = Once::new();

pub(crate) fn init(base: usize) {
    TEST.call_once(|| QemuTest(RISCV64::new(base as _)));
}

pub(crate) fn get() -> &'static QemuTest {
    TEST.wait()
}

impl Reset for QemuTest {
    fn system_reset(&self, reset_type: u32, reset_reason: u32) -> SbiRet {
        match reset_type {
            RESET_TYPE_SHUTDOWN => match reset_reason {
                RESET_REASON_NO_REASON => self.0.exit_success(),
                RESET_REASON_SYSTEM_FAILURE => self.0.exit_failure(),
                value => match u32::try_from(value) {
                    Ok(code) => self.0.exit(code),
                    Err(_) => SbiRet::invalid_param(),
                },
            },
            RESET_TYPE_COLD_REBOOT | RESET_TYPE_WARM_REBOOT => SbiRet::ok(0),
            _ => SbiRet::invalid_param(),
        }
    }
}
